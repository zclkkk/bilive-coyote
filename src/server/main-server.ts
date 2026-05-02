import type { ConfigStore } from "../config/store"
import type { EventBus } from "../engine/event-bus"
import type { CoyoteServer } from "../coyote/server"
import type { StrengthManager } from "../engine/strength-manager"
import type { BilibiliService } from "../bilibili/service"
import { ValidationError } from "../config/schema"
import { createRouter, matchRoute } from "./router"
import panel from "../../public/index.html"

export class MainServer {
  private config: ConfigStore
  private eventBus: EventBus
  private strengthMgr: StrengthManager
  private routes: Map<string, (req: Request, url: URL) => Promise<Response> | Response>
  private panelClients: Set<any> = new Set()
  private server: any = null

  constructor(
    config: ConfigStore,
    eventBus: EventBus,
    coyote: CoyoteServer,
    strengthMgr: StrengthManager,
    bilibili: BilibiliService,
  ) {
    this.config = config
    this.eventBus = eventBus
    this.strengthMgr = strengthMgr
    this.routes = createRouter(config, coyote, strengthMgr, bilibili)
    this.setupEventForwarding()
  }

  async start(): Promise<void> {
    const { httpPort, host } = this.config.server

    this.server = Bun.serve({
      port: httpPort,
      hostname: host,
      routes: {
        "/": panel,
      },
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

    return new Response("Not found", { status: 404 })
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
