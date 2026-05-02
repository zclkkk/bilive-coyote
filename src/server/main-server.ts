import { resolve, extname } from "path"
import { existsSync } from "fs"
import type { ConfigStore } from "../config/store"
import type { EventBus } from "../engine/event-bus"
import type { CoyoteServer } from "../coyote/server"
import type { StrengthManager } from "../engine/strength-manager"
import type { BilibiliClient } from "../bilibili/api"
import type { DanmakuWS } from "../bilibili/danmaku-ws"
import { ValidationError } from "../config/schema"
import { createRouter, matchRoute } from "./router"

const MIME_TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".json": "application/json",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
}

// 静态文件目录：环境变量 > cwd/public > 源码相邻目录 (开发环境)
function resolvePublicDir(): string {
  const envDir = process.env.PUBLIC_DIR
  if (envDir && envDir.length > 0) return resolve(envDir)
  const cwdPublic = resolve(process.cwd(), "public")
  if (existsSync(cwdPublic)) return cwdPublic
  return resolve(import.meta.dir, "../../public")
}

const PUBLIC_DIR = resolvePublicDir()

export class MainServer {
  private config: ConfigStore
  private eventBus: EventBus
  private coyote: CoyoteServer
  private strengthMgr: StrengthManager
  private bilibili: BilibiliClient
  private danmaku: DanmakuWS
  private routes: Map<string, (req: Request, url: URL) => Promise<Response> | Response>
  private panelClients: Set<any> = new Set()
  private server: any = null

  constructor(
    config: ConfigStore,
    eventBus: EventBus,
    coyote: CoyoteServer,
    strengthMgr: StrengthManager,
    bilibili: BilibiliClient,
    danmaku: DanmakuWS,
  ) {
    this.config = config
    this.eventBus = eventBus
    this.coyote = coyote
    this.strengthMgr = strengthMgr
    this.bilibili = bilibili
    this.danmaku = danmaku
    this.routes = createRouter(config, eventBus, coyote, strengthMgr, bilibili)
    this.setupEventForwarding()
  }

  async start(): Promise<void> {
    const { httpPort, host } = this.config.server

    this.server = Bun.serve({
      port: httpPort,
      hostname: host,
      fetch: (req, server) => this.handleRequest(req, server),
      websocket: {
        open: (ws) => this.onWsOpen(ws),
        message: () => {},
        close: (ws) => this.onWsClose(ws),
      },
    })

    console.log(`[Server] HTTP + WS started on http://${host}:${httpPort}`)
  }

  private async handleRequest(req: Request, server: any): Promise<Response> {
    const url = new URL(req.url)

    if (url.pathname === "/ws/panel") {
      if (server.upgrade(req, { data: { channel: "panel" } } as any)) return new Response(null)
      return new Response("WS upgrade failed", { status: 500 })
    }

    if (url.pathname.startsWith("/api/")) {
      const handler = matchRoute(this.routes, req.method, url.pathname)
      if (handler) {
        try {
          return await handler(req, url)
        } catch (e: any) {
          if (e instanceof ValidationError) {
            return Response.json({ error: e.message }, { status: 400 })
          }
          return Response.json({ error: e.message }, { status: 500 })
        }
      }
      return Response.json({ error: "Not found" }, { status: 404 })
    }

    return this.serveStatic(url.pathname)
  }

  private serveStatic(pathname: string): Response {
    let filePath = resolve(PUBLIC_DIR, pathname.slice(1) || "index.html")

    if (!existsSync(filePath)) {
      filePath = resolve(PUBLIC_DIR, "index.html")
    }

    const ext = extname(filePath)
    const mime = MIME_TYPES[ext] || "application/octet-stream"
    const file = Bun.file(filePath)

    if (!file.size) {
      return new Response("Not Found", { status: 404 })
    }

    return new Response(file, { headers: { "Content-Type": mime } })
  }

  private onWsOpen(ws: any): void {
    const channel = ws.data?.channel
    if (channel === "panel") {
      this.panelClients.add(ws)
    }
  }

  private onWsClose(ws: any): void {
    this.panelClients.delete(ws)
  }

  private setupEventForwarding(): void {
    this.eventBus.on("gift:log", (data) => {
      this.broadcast({ type: "gift", data })
    })

    this.eventBus.on("strength:change", (data) => {
      this.broadcast({
        type: "strength",
        data: {
          channel: data.channel,
          value: this.strengthMgr.getStrength(data.channel),
          source: data.source,
        },
      })
    })

    this.eventBus.on("bilibili:status", (data) => {
      this.broadcast({ type: "bilibili:status", data })
    })

    this.eventBus.on("coyote:status", (data) => {
      this.broadcast({ type: "coyote:status", data })
    })
  }

  private broadcast(data: any): void {
    const msg = JSON.stringify(data)
    for (const ws of this.panelClients) {
      try {
        ws.send(msg)
      } catch (e) {
        console.error("[Server] Panel broadcast send failed:", e)
      }
    }
  }

  stop(): void {
    if (this.server) this.server.stop()
  }
}
