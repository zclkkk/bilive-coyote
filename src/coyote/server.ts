import { randomUUID } from "crypto"
import { networkInterfaces } from "os"
import QRCode from "qrcode"
import type { Server, ServerWebSocket } from "bun"
import { buildMessage, parseMessage, parseStrengthFeedback } from "./message"
import { ErrCode } from "./error-codes"
import type { EventBus } from "../engine/event-bus"
import type { ConfigStore } from "../config/store"

type AppWs = ServerWebSocket<{ id: string }>

function getLANIP(): string {
  const interfaces = networkInterfaces()
  for (const name of Object.keys(interfaces)) {
    for (const iface of interfaces[name] || []) {
      if (iface.family === "IPv4" && !iface.internal) return iface.address
    }
  }
  return "127.0.0.1"
}

const INITIAL_STRENGTH = { a: 0, b: 0, limitA: 200, limitB: 200 }

export class CoyoteServer {
  private readonly bridgeId = randomUUID()
  private heartbeatInterval: ReturnType<typeof setInterval> | null = null
  private eventBus: EventBus
  private config: ConfigStore
  private appWs: AppWs | null = null
  private appClientId: string | null = null
  private currentStrength = { ...INITIAL_STRENGTH }
  private server: Server<{ id: string }> | null = null

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config
    this.eventBus = eventBus
  }

  async start(): Promise<void> {
    const port = this.config.coyote.wsPort

    this.server = Bun.serve<{ id: string }>({
      port,
      fetch: (req, server) => {
        const url = new URL(req.url)
        const bridgeId = url.pathname.slice(1)
        if (bridgeId !== this.bridgeId) return new Response("Invalid bridge ID", { status: 404 })
        if (server.upgrade(req, { data: { id: randomUUID() } })) return undefined
        return new Response("Coyote WS only", { status: 400 })
      },
      websocket: {
        open: (ws) => this.onOpen(ws),
        message: (ws, data) => this.onMessage(ws, data),
        close: (ws) => this.onClose(ws),
      },
    })

    this.heartbeatInterval = setInterval(() => this.sendHeartbeat(), 30000)

    console.log(`[Coyote] WS server started on port ${port}`)
    console.log(`[Coyote] Bridge ID: ${this.bridgeId}`)
  }

  private onOpen(ws: AppWs): void {
    ws.send(buildMessage("bind", ws.data.id, "", "targetId"))
    console.log(`[Coyote] App socket connected: ${ws.data.id}`)
  }

  private onMessage(ws: AppWs, rawData: string | Buffer): void {
    const data = typeof rawData === "string" ? rawData : new TextDecoder().decode(rawData)
    const parsed = parseMessage(data)

    if (!parsed.ok) {
      ws.send(buildMessage("msg", "", "", parsed.code))
      return
    }

    const msg = parsed.message
    if (msg.type === "bind") {
      this.handleBind(ws, msg)
      return
    }

    if (ws !== this.appWs || msg.clientId !== this.bridgeId || msg.targetId !== this.appClientId) {
      ws.send(buildMessage("error", msg.clientId, msg.targetId, ErrCode.NOT_PAIRED))
      return
    }

    this.handleAppMessage(msg.message)
  }

  private handleBind(ws: AppWs, msg: { clientId: string; targetId: string }): void {
    const appId = ws.data.id
    if (msg.clientId !== this.bridgeId) {
      ws.send(buildMessage("bind", msg.clientId, msg.targetId, ErrCode.INVALID_QR_CLIENT_ID))
      return
    }
    if (msg.targetId !== appId) {
      ws.send(buildMessage("bind", msg.clientId, msg.targetId, ErrCode.NO_TARGET_ID))
      return
    }

    if (this.appWs && this.appWs !== ws) this.appWs.close()

    this.appWs = ws
    this.appClientId = appId
    this.currentStrength = { ...INITIAL_STRENGTH }

    ws.send(buildMessage("bind", this.bridgeId, appId, ErrCode.SUCCESS))
    console.log(`[Coyote] Paired with app: ${appId}`)
    this.emitStatus()
  }

  private handleAppMessage(message: string): void {
    const feedback = parseStrengthFeedback(message)
    if (!feedback) return

    this.currentStrength = feedback
    this.emitStatus()
  }

  private onClose(ws: AppWs): void {
    if (ws !== this.appWs) return

    const appId = this.appClientId
    this.appWs = null
    this.appClientId = null
    this.currentStrength = { ...INITIAL_STRENGTH }

    console.log(`[Coyote] App disconnected: ${appId}`)
    this.emitStatus()
  }

  private sendHeartbeat(): void {
    if (!this.appWs || !this.appClientId) return
    this.appWs.send(buildMessage("heartbeat", this.appClientId, this.bridgeId, ErrCode.SUCCESS))
  }

  sendStrength(channel: "A" | "B", mode: number, value: number): void {
    const channelNum = channel === "A" ? 1 : 2
    this.sendAppCommand(`strength-${channelNum}+${mode}+${value}`)
  }

  sendClear(channel: "A" | "B"): void {
    const channelNum = channel === "A" ? 1 : 2
    this.sendAppCommand(`clear-${channelNum}`)
  }

  async getQRCodeBase64(): Promise<string | null> {
    const host = this.config.server.host === "0.0.0.0" ? getLANIP() : this.config.server.host
    const wsUrl = `ws://${host}:${this.config.coyote.wsPort}/${this.bridgeId}`
    const qrContent = `https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#${wsUrl}`
    try {
      return await QRCode.toDataURL(qrContent)
    } catch {
      return null
    }
  }

  getStatus() {
    return {
      paired: !!this.appWs,
      strengthA: this.currentStrength.a,
      strengthB: this.currentStrength.b,
      limitA: this.currentStrength.limitA,
      limitB: this.currentStrength.limitB,
    }
  }

  stop(): void {
    if (this.heartbeatInterval) clearInterval(this.heartbeatInterval)
    if (this.appWs) this.appWs.close()
    if (this.server) this.server.stop()
  }

  private sendAppCommand(command: string): void {
    if (!this.appWs || !this.appClientId) return
    this.appWs.send(buildMessage("msg", this.bridgeId, this.appClientId, command))
  }

  private emitStatus(): void {
    this.eventBus.emit("coyote:status", {
      paired: !!this.appWs,
      strengthA: this.currentStrength.a,
      strengthB: this.currentStrength.b,
      limitA: this.currentStrength.limitA,
      limitB: this.currentStrength.limitB,
    })
  }
}
