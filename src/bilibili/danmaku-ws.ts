import { inflateSync, brotliDecompressSync } from "zlib"
import type { EventBus } from "../engine/event-bus"
import type { GiftEvent } from "../engine/event-bus"
import type { BilibiliClient } from "./api"

const WS_OP_HEARTBEAT = 2
const WS_OP_HEARTBEAT_REPLY = 3
const WS_OP_MESSAGE = 5
const WS_OP_AUTH = 7
const WS_OP_CONNECT_SUCCESS = 8

const WS_HEADER_LEN = 16
const WS_BODY_PROTOCOL_VERSION_DEFLATE = 2
const WS_BODY_PROTOCOL_VERSION_BROTLI = 3

const HEARTBEAT_INTERVAL_MS = 20000
const RECONNECT_BASE_MS = 3000
const RECONNECT_MAX_MS = 60000
const MAX_RECONNECT_ATTEMPTS = 5

const CMD_OPEN_PLATFORM_GIFT = "LIVE_OPEN_PLATFORM_SEND_GIFT"

export class DanmakuWS {
  private ws: WebSocket | null = null
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null
  private eventBus: EventBus
  private bilibili: BilibiliClient
  private roomId: number | null = null

  private wssLinks: string[] = []
  private authBody: string = ""
  private wssIndex: number = 0
  private reconnectAttempts: number = 0
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private intentionalDisconnect: boolean = false

  constructor(bilibili: BilibiliClient, eventBus: EventBus) {
    this.bilibili = bilibili
    this.eventBus = eventBus
  }

  async connect(wssLinks: string[], authBody: string): Promise<void> {
    let auth: any
    try {
      auth = JSON.parse(authBody)
    } catch (e: any) {
      console.error("[Danmaku] Invalid auth_body:", e.message)
      this.eventBus.emit("bilibili:status", { connected: false, error: "auth_body 格式错误" })
      return
    }
    const url = wssLinks[0]
    if (!url) {
      console.error("[Danmaku] No wss_link provided")
      this.eventBus.emit("bilibili:status", { connected: false, error: "wss_link 为空" })
      return
    }

    this.wssLinks = wssLinks
    this.authBody = authBody
    this.wssIndex = 0
    this.reconnectAttempts = 0
    this.intentionalDisconnect = false
    this.roomId = typeof auth.roomid === "number" ? auth.roomid : null

    this.doConnect(url, auth)
  }

  private doConnect(url: string, auth: any): void {
    console.log(`[Danmaku] Connecting to ${url}, room: ${this.roomId}`)
    this.ws = new WebSocket(url)

    this.ws.onopen = () => {
      console.log("[Danmaku] Connected")
      this.sendAuth(auth)
    }

    this.ws.onmessage = (event) => {
      const data = typeof event.data === "string"
        ? new TextEncoder().encode(event.data)
        : event.data as ArrayBuffer
      this.handleData(new Uint8Array(data))
    }

    this.ws.onclose = () => {
      console.log("[Danmaku] Disconnected")
      this.cleanup()
      if (!this.intentionalDisconnect) {
        this.tryReconnect()
      } else {
        this.eventBus.emit("bilibili:status", { connected: false })
      }
    }

    this.ws.onerror = (e) => {
      console.error("[Danmaku] Error:", e)
    }
  }

  private tryReconnect(): void {
    if (this.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
      console.error(`[Danmaku] Reconnect failed after ${MAX_RECONNECT_ATTEMPTS} attempts`)
      this.eventBus.emit("bilibili:status", {
        connected: false,
        error: `弹幕连接断开，已重试 ${MAX_RECONNECT_ATTEMPTS} 次仍失败，请手动重新连接`,
      })
      return
    }

    this.reconnectAttempts++
    this.wssIndex = (this.wssIndex + 1) % this.wssLinks.length
    const url = this.wssLinks[this.wssIndex]
    const delay = Math.min(RECONNECT_BASE_MS * Math.pow(2, this.reconnectAttempts - 1), RECONNECT_MAX_MS)

    console.log(`[Danmaku] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS}, url index ${this.wssIndex})`)

    let auth: any
    try {
      auth = JSON.parse(this.authBody)
    } catch (e: any) {
      console.error("[Danmaku] Invalid stored auth_body:", e.message)
      this.eventBus.emit("bilibili:status", { connected: false, error: "重连凭证失效，请手动重新连接" })
      return
    }

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      this.doConnect(url, auth)
    }, delay)
  }

  private sendAuth(auth: any): void {
    const authPayload = JSON.stringify({
      key: auth.key,
      group: auth.group,
      roomid: auth.roomid,
      protoover: auth.protoover || 2,
      uid: auth.uid || 0,
    })
    const packet = this.buildPacket(WS_OP_AUTH, authPayload)
    this.ws?.send(packet)
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(this.buildPacket(WS_OP_HEARTBEAT, ""))
      }
    }, HEARTBEAT_INTERVAL_MS)
  }

  private handleData(buf: Uint8Array): void {
    let offset = 0
    while (offset < buf.length) {
      if (buf.length - offset < WS_HEADER_LEN) break

      const view = new DataView(buf.buffer, buf.byteOffset + offset)
      const totalLen = view.getUint32(0)
      const headerLen = view.getUint16(4)
      const protover = view.getUint16(6)
      const op = view.getUint32(8)

      if (totalLen > buf.length - offset) break

      const body = buf.slice(offset + headerLen, offset + totalLen)

      switch (op) {
        case WS_OP_CONNECT_SUCCESS:
          console.log("[Danmaku] Auth success, connected to room")
          this.reconnectAttempts = 0
          this.startHeartbeat()
          this.eventBus.emit("bilibili:status", { connected: true, roomId: this.roomId ?? undefined })
          break

        case WS_OP_HEARTBEAT_REPLY:
          break

        case WS_OP_MESSAGE:
          if (protover === WS_BODY_PROTOCOL_VERSION_DEFLATE) {
            try {
              const inflated = inflateSync(body)
              this.handleData(new Uint8Array(inflated))
            } catch (e) {
              console.error("[Danmaku] Inflate error:", e)
            }
          } else if (protover === WS_BODY_PROTOCOL_VERSION_BROTLI) {
            try {
              const decompressed = brotliDecompressSync(body)
              this.handleData(new Uint8Array(decompressed))
            } catch (e) {
              console.error("[Danmaku] Brotli error:", e)
            }
          } else {
            this.parseMessage(body)
          }
          break
      }

      offset += totalLen
    }
  }

  private parseMessage(body: Uint8Array): void {
    try {
      const text = new TextDecoder().decode(body)
      const msg = JSON.parse(text)

      if (msg.cmd === CMD_OPEN_PLATFORM_GIFT && msg.data) {
        const d = msg.data
        const coinType = d.paid === true ? "gold" : "silver"
        const giftEvent: GiftEvent = {
          giftId: d.gift_id ?? d.giftId ?? 0,
          giftName: d.gift_name ?? d.giftName ?? "",
          coinType,
          totalCoin: d.price ?? d.total_coin ?? 0,
          num: d.gift_num ?? d.num ?? 1,
          uid: d.uid ?? 0,
          uname: d.uname ?? d.username ?? "",
          timestamp: d.timestamp ?? Math.floor(Date.now() / 1000),
        }
        this.eventBus.emit("gift", giftEvent)
      }
    } catch (e) {
      console.error("[Danmaku] Failed to parse message:", e)
    }
  }

  private buildPacket(op: number, body: string): ArrayBuffer {
    const bodyBytes = new TextEncoder().encode(body)
    const totalLen = WS_HEADER_LEN + bodyBytes.length
    const buf = new ArrayBuffer(totalLen)
    const view = new DataView(buf)
    const bodyOffset = WS_HEADER_LEN

    view.setUint32(0, totalLen)
    view.setUint16(4, WS_HEADER_LEN)
    view.setUint16(6, 1)
    view.setUint32(8, op)
    view.setUint32(12, 1)

    new Uint8Array(buf, bodyOffset).set(bodyBytes)
    return buf
  }

  private cleanup(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
  }

  disconnect(): void {
    this.intentionalDisconnect = true
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    this.cleanup()
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
  }
}
