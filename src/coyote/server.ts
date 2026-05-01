import { PairingManager } from "./pairing"
import { parseMessage, buildMessage, convertFrontendType, parseStrengthFeedback, isValidChannel } from "./message"
import { PulseTimerManager } from "./pulse-timer"
import { ErrCode } from "./error-codes"
import type { EventBus } from "../engine/event-bus"
import type { ConfigStore } from "../config/store"
import QRCode from "qrcode"
import { networkInterfaces } from "os"

function getLANIP(): string {
  const interfaces = networkInterfaces()
  for (const name of Object.keys(interfaces)) {
    for (const iface of interfaces[name] || []) {
      if (iface.family === "IPv4" && !iface.internal) {
        return iface.address
      }
    }
  }
  return "127.0.0.1"
}

interface ClientInfo {
  ws: any
  id: string
  isApp: boolean
}

export class CoyoteServer {
  private pairing = new PairingManager()
  private clients: Map<string, ClientInfo> = new Map()
  private pulseTimers: PulseTimerManager
  private heartbeatInterval: ReturnType<typeof setInterval> | null = null
  private eventBus: EventBus
  private config: ConfigStore
  private frontClientId: string | null = null
  private appClientId: string | null = null
  private currentStrength = { a: 0, b: 0, limitA: 200, limitB: 200 }
  private server: any = null
  private virtualClientId: string | null = null
  private lastHeartbeatAt: Map<string, number> = new Map()

  private static readonly STALE_CLIENT_TIMEOUT_MS = 90_000

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config
    this.eventBus = eventBus
    this.pulseTimers = new PulseTimerManager(
      (clientId, targetId, msg) => this.sendTo(clientId, targetId, msg),
      (clientId, targetId, msg) => this.sendNotify(clientId, targetId, msg),
    )
  }

  async start(): Promise<void> {
    const port = this.config.coyote.wsPort

    this.virtualClientId = this.pairing.generateId()
    this.frontClientId = this.virtualClientId
    this.clients.set(this.virtualClientId, {
      ws: { send: () => {}, close: () => {} } as any,
      id: this.virtualClientId,
      isApp: false,
    })

    this.server = Bun.serve({
      port,
      fetch(req: any, server: any) {
        const url = new URL(req.url)
        const clientId = url.pathname.slice(1) || ""
        if (server.upgrade(req, { data: { qrClientId: clientId } } as any)) return new Response(null)
        return new Response("Coyote WS only", { status: 400 })
      },
      websocket: {
        open: (ws: any) => this.onOpen(ws),
        message: (ws: any, data: any) => this.onMessage(ws, data),
        close: (ws: any) => this.onClose(ws),
      },
    } as any)

    this.heartbeatInterval = setInterval(() => this.sendHeartbeats(), 30000)

    console.log(`[Coyote] WS server started on port ${port}`)
    console.log(`[Coyote] Virtual frontend ID: ${this.virtualClientId}`)
  }

  private onOpen(ws: any): void {
    const id = this.pairing.generateId()
    ws.data = { ...ws.data, id }
    this.clients.set(id, { ws, id, isApp: false })
    this.lastHeartbeatAt.set(id, Date.now())
    const bindMsg = buildMessage("bind", id, "", "targetId")
    ws.send(bindMsg)
    console.log(`[Coyote] Client connected: ${id}`)
  }

  private onMessage(ws: any, rawData: any): void {
    const data = typeof rawData === "string" ? rawData : new TextDecoder().decode(rawData)

    const msg = parseMessage(data)
    if (!msg) {
      ws.send(buildMessage("msg", "", "", ErrCode.INVALID_JSON))
      return
    }

    if (msg.type === "bind" && msg.targetId) {
      const senderInfo = this.clients.get(msg.targetId)
      if (!senderInfo || senderInfo.ws !== ws) {
        ws.send(buildMessage("bind", msg.clientId, msg.targetId, ErrCode.PEER_OFFLINE))
        return
      }
      this.handleBind(ws, msg)
      return
    }

    const fromFrontend = this.clients.get(msg.clientId)?.ws === ws
    const fromApp = this.clients.get(msg.targetId)?.ws === ws

    if (!fromFrontend && !fromApp) {
      ws.send(buildMessage("msg", msg.clientId, msg.targetId, ErrCode.PEER_OFFLINE))
      return
    }

    // 必须验证 clientId ↔ targetId 是互相配对的，否则 A-B 配对里的 A 可以伪造发给 D 的消息
    if (!this.pairing.isPairedWith(msg.clientId, msg.targetId)) {
      ws.send(buildMessage("error", msg.clientId, msg.targetId, ErrCode.NOT_PAIRED))
      return
    }

    if (fromFrontend) {
      this.handleFrontendMessage(ws, msg)
    } else {
      this.handleAppMessage(ws, msg)
    }
  }

  private handleFrontendMessage(ws: any, msg: any): void {
    const partnerId = this.pairing.getPartnerId(msg.clientId)
    if (!partnerId) {
      ws.send(buildMessage("error", msg.clientId, msg.targetId, ErrCode.PEER_OFFLINE))
      return
    }

    if (typeof msg.type === "number" && msg.type >= 1 && msg.type <= 3) {
      const channel = (msg as any).channel || 1
      const strength = (msg as any).strength || 0
      const result = convertFrontendType(msg.type, channel, strength)
      if (result) {
        this.sendTo(msg.clientId, partnerId, result.message)
      }
      return
    }

    if (msg.type === 4) {
      this.sendTo(msg.clientId, partnerId, msg.message)
      return
    }

    if (msg.type === "clientMsg") {
      this.handleClientMsg(msg, partnerId)
      return
    }

    this.sendTo(msg.clientId, partnerId, msg.message)
  }

  private handleAppMessage(ws: any, msg: any): void {
    const frontId = msg.clientId
    const frontWs = this.clients.get(frontId)?.ws

    if (frontWs && frontId !== this.virtualClientId) {
      frontWs.send(buildMessage("msg", msg.targetId, msg.clientId, msg.message))
    }

    const feedback = parseStrengthFeedback(msg.message)
    if (feedback) {
      this.currentStrength = feedback
      this.eventBus.emit("coyote:status", {
        paired: true,
        strengthA: feedback.a,
        strengthB: feedback.b,
        limitA: feedback.limitA,
        limitB: feedback.limitB,
        clientCount: this.clients.size - 1,
      })
    }
  }

  private handleBind(ws: any, msg: { type: string | number; clientId: string; targetId: string; message: string }): void {
    const frontId = msg.clientId
    const appId = msg.targetId

    const frontExists = this.clients.has(frontId)
    if (!frontExists) {
      ws.send(buildMessage("bind", frontId, appId, ErrCode.TARGET_NOT_EXIST))
      return
    }

    const ok = this.pairing.pair(frontId, appId)
    if (!ok) {
      ws.send(buildMessage("bind", frontId, appId, ErrCode.ALREADY_BOUND))
      return
    }

    const appClientInfo = this.clients.get(appId)
    if (appClientInfo) appClientInfo.isApp = true

    this.appClientId = appId
    this.frontClientId = frontId

    const appWs = this.clients.get(appId)?.ws
    const frontWs = this.clients.get(frontId)?.ws

    const bindSuccess = buildMessage("bind", frontId, appId, ErrCode.SUCCESS)
    if (appWs) appWs.send(bindSuccess)
    if (frontWs && frontId !== this.virtualClientId) frontWs.send(bindSuccess)

    console.log(`[Coyote] Paired: frontend=${frontId} <-> app=${appId}`)
    this.eventBus.emit("coyote:status", {
      paired: true,
      strengthA: this.currentStrength.a,
      strengthB: this.currentStrength.b,
      limitA: this.currentStrength.limitA,
      limitB: this.currentStrength.limitB,
      clientCount: this.clients.size - 1,
    })
  }

  private handleClientMsg(msg: any, partnerId: string): void {
    const channel = msg.channel
    if (!isValidChannel(channel)) {
      const clientInfo = this.clients.get(msg.clientId)
      if (clientInfo) clientInfo.ws.send(buildMessage("error", msg.clientId, msg.targetId, ErrCode.CHANNEL_REQUIRED))
      return
    }

    const time = msg.time || 5
    const key = `${msg.clientId}-${channel}`

    let hexArray: string[] = []
    try {
      const msgContent = msg.message as string
      const colonIdx = msgContent.indexOf(":")
      const jsonStr = colonIdx >= 0 ? msgContent.substring(colonIdx + 1) : msgContent
      hexArray = JSON.parse(jsonStr)
    } catch {
      hexArray = [msg.message]
    }

    this.pulseTimers.startPulse(key, msg.clientId, partnerId, channel, hexArray, time)
  }

  private onClose(ws: any): void {
    const id = ws.data?.id
    if (!id) return

    const partnerId = this.pairing.unpair(id)
    this.pulseTimers.stopPulseByClient(id)

    this.clients.delete(id)
    this.lastHeartbeatAt.delete(id)

    if (id === this.appClientId) this.appClientId = null
    if (id === this.frontClientId && id !== this.virtualClientId) this.frontClientId = this.virtualClientId

    if (partnerId && partnerId !== this.virtualClientId) {
      const partner = this.clients.get(partnerId)
      if (partner) {
        partner.ws.send(buildMessage("break", id, partnerId, ErrCode.PEER_DISCONNECTED))
        setTimeout(() => {
          const p = this.clients.get(partnerId)
          if (p) p.ws.close()
          this.clients.delete(partnerId)
        }, 1000)
      }
    }

    console.log(`[Coyote] Client disconnected: ${id}`)
    this.eventBus.emit("coyote:status", {
      paired: !!this.appClientId,
      strengthA: 0,
      strengthB: 0,
      limitA: this.currentStrength.limitA,
      limitB: this.currentStrength.limitB,
      clientCount: this.clients.size - 1,
    })
  }

  private sendTo(fromId: string, toId: string, message: string): void {
    const client = this.clients.get(toId)
    if (client) {
      client.ws.send(buildMessage("msg", fromId, toId, message))
    }
  }

  private sendNotify(clientId: string, targetId: string, message: string): void {
    const client = this.clients.get(clientId)
    if (client) {
      client.ws.send(buildMessage("notify", clientId, targetId, message))
    }
  }

  private sendHeartbeats(): void {
    for (const [id, client] of this.clients) {
      if (id === this.virtualClientId) continue
      try {
        const partnerId = this.pairing.getPartnerId(id) || ""
        client.ws.send(buildMessage("heartbeat", id, partnerId, ErrCode.SUCCESS))
        this.lastHeartbeatAt.set(id, Date.now())
      } catch (e) {
        console.error(`[Coyote] Heartbeat send failed for ${id}:`, e)
      }
    }
    this.cleanupStaleClients()
  }

  private cleanupStaleClients(): void {
    const now = Date.now()
    for (const [id, lastSeen] of this.lastHeartbeatAt) {
      if (id === this.virtualClientId) continue
      if (now - lastSeen <= CoyoteServer.STALE_CLIENT_TIMEOUT_MS) continue

      console.log(`[Coyote] Cleaning up stale client: ${id} (last seen ${now - lastSeen}ms ago)`)
      const clientInfo = this.clients.get(id)
      if (clientInfo) {
        try { clientInfo.ws.close() } catch {}
        this.onClose(clientInfo.ws)
      }
    }
  }

  sendStrength(channel: "A" | "B", mode: number, value: number): void {
    if (!this.appClientId) return
    const fromId = this.frontClientId || this.virtualClientId || ""
    const ch = channel === "A" ? 1 : 2
    const msg = `strength-${ch}+${mode}+${value}`
    this.sendTo(fromId, this.appClientId, msg)
  }

  sendClear(channel: "A" | "B"): void {
    if (!this.appClientId) return
    const fromId = this.frontClientId || this.virtualClientId || ""
    const ch = channel === "A" ? 1 : 2
    this.sendTo(fromId, this.appClientId, `clear-${ch}`)
  }

  async getQRCodeBase64(): Promise<string | null> {
    if (!this.frontClientId) return null
    const host = this.config.server.host === "0.0.0.0" ? getLANIP() : this.config.server.host
    const wsUrl = `ws://${host}:${this.config.coyote.wsPort}/${this.frontClientId}`
    const qrContent = `https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#${wsUrl}`
    try {
      return await QRCode.toDataURL(qrContent)
    } catch {
      return null
    }
  }

  getStatus() {
    return {
      paired: !!this.appClientId,
      clientCount: this.clients.size - 1,
      strengthA: this.currentStrength.a,
      strengthB: this.currentStrength.b,
      limitA: this.currentStrength.limitA,
      limitB: this.currentStrength.limitB,
    }
  }

  stop(): void {
    if (this.heartbeatInterval) clearInterval(this.heartbeatInterval)
    this.pulseTimers.stopAll()
    if (this.server) this.server.stop()
  }
}
