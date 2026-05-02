import { randomUUID } from "crypto"
import { networkInterfaces } from "os"
import QRCode from "qrcode"
import { buildMessage, parseMessage, parseStrengthFeedback } from "./message"
import { ErrCode } from "./error-codes"
import type { EventBus } from "../engine/event-bus"
import type { ConfigStore } from "../config/store"

function getLANIP(): string {
  const interfaces = networkInterfaces()
  for (const name of Object.keys(interfaces)) {
    for (const iface of interfaces[name] || []) {
      if (iface.family === "IPv4" && !iface.internal) return iface.address
    }
  }
  return "127.0.0.1"
}

export class CoyoteServer {
  private readonly bridgeId = randomUUID()
  private heartbeatInterval: ReturnType<typeof setInterval> | null = null
  private eventBus: EventBus
  private config: ConfigStore
  private appWs: any = null
  private appClientId: string | null = null
  private currentStrength = { a: 0, b: 0, limitA: 200, limitB: 200 }
  private server: any = null

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config
    this.eventBus = eventBus
  }

  async start(): Promise<void> {
    const port = this.config.coyote.wsPort

    this.server = Bun.serve({
      port,
      fetch: (req: Request, server: any) => {
        const url = new URL(req.url)
        const bridgeId = url.pathname.slice(1)
        if (bridgeId !== this.bridgeId) return new Response("Invalid bridge ID", { status: 404 })
        if (server.upgrade(req)) return new Response(null)
        return new Response("Coyote WS only", { status: 400 })
      },
      websocket: {
        open: (ws: any) => this.onOpen(ws),
        message: (ws: any, data: any) => this.onMessage(ws, data),
        close: (ws: any) => this.onClose(ws),
      },
    } as any)

    this.heartbeatInterval = setInterval(() => this.sendHeartbeat(), 30000)

    console.log(`[Coyote] WS server started on port ${port}`)
    console.log(`[Coyote] Bridge ID: ${this.bridgeId}`)
  }

  private onOpen(ws: any): void {
    const appId = randomUUID()
    ws.data = { id: appId }
    ws.send(buildMessage("bind", appId, "", "targetId"))
    console.log(`[Coyote] App socket connected: ${appId}`)
  }

  private onMessage(ws: any, rawData: any): void {
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

  private handleBind(ws: any, msg: { clientId: string; targetId: string }): void {
    const appId = ws.data?.id
    if (msg.clientId !== this.bridgeId) {
      ws.send(buildMessage("bind", msg.clientId, msg.targetId, ErrCode.INVALID_QR_CLIENT_ID))
      return
    }
    if (!appId || msg.targetId !== appId) {
      ws.send(buildMessage("bind", msg.clientId, msg.targetId, ErrCode.NO_TARGET_ID))
      return
    }

    if (this.appWs && this.appWs !== ws) this.appWs.close()

    this.appWs = ws
    this.appClientId = appId
    this.currentStrength = { a: 0, b: 0, limitA: 200, limitB: 200 }

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

  private onClose(ws: any): void {
    if (ws !== this.appWs) return

    const appId = this.appClientId
    this.appWs = null
    this.appClientId = null
    this.currentStrength = { a: 0, b: 0, limitA: 200, limitB: 200 }

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
