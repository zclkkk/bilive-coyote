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

const CMD_OPEN_PLATFORM_GIFT = "LIVE_OPEN_PLATFORM_SEND_GIFT"

export class DanmakuWS {
  private ws: WebSocket | null = null
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null
  private eventBus: EventBus
  private bilibili: BilibiliClient
  private roomId: number | null = null

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
    this.roomId = typeof auth.roomid === "number" ? auth.roomid : null

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
      this.eventBus.emit("bilibili:status", { connected: false })
    }

    this.ws.onerror = (e) => {
      console.error("[Danmaku] Error:", e)
      this.eventBus.emit("bilibili:status", { connected: false, error: "WebSocket error" })
    }
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
        // 开放平台使用 snake_case；paid=true 才是付费礼物 (金瓜子)
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
    } catch {}
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
    this.cleanup()
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
  }
}
