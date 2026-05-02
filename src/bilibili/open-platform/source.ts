import { BilibiliLiveSocket, type LiveSocketStatus } from "../live-socket"
import { parseOpenPlatformGift } from "./parser"
import { signOpenPlatformRequest } from "./signer"
import type { BilibiliSource, BilibiliStatus, OpenPlatformStartInput } from "../types"
import type { ConfigStore } from "../../config/store"
import type { EventBus } from "../../engine/event-bus"

const BASE_URL = "https://live-open.biliapi.com"

interface OpenPlatformCredentials {
  appKey: string
  appSecret: string
}

export class OpenPlatformSource implements BilibiliSource<"open-platform"> {
  readonly type = "open-platform" as const

  private config: ConfigStore
  private eventBus: EventBus
  private socket: BilibiliLiveSocket
  private credentials: OpenPlatformCredentials = { appKey: "", appSecret: "" }
  private appId = 0
  private gameId: string | null = null
  private httpHeartbeatTimer: ReturnType<typeof setInterval> | null = null
  private roomId: number | null = null
  private socketStatus: LiveSocketStatus = { connected: false }

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config
    this.eventBus = eventBus
    this.socket = new BilibiliLiveSocket()
  }

  async start(input: OpenPlatformStartInput): Promise<void> {
    if (this.gameId) await this.stop()

    const defaults = this.config.bilibili.openPlatform
    const appKey = input.appKey ?? defaults.appKey
    const appSecret = input.appSecret ?? defaults.appSecret
    const code = input.code ?? defaults.code
    const appId = input.appId ?? defaults.appId

    if (!appKey || !appSecret || !code || !appId) {
      throw new Error("code, appId, appKey and appSecret required")
    }

    this.credentials = { appKey, appSecret }
    this.appId = appId

    await this.clearStaleGame(appId)

    const data = await this.request("/v2/app/start", { code, app_id: appId })
    if (data.code === 7002) {
      throw new Error("直播间已有互动玩法会话，请先结束已有会话后重试")
    }
    if (data.code === 7001) {
      throw new Error("请求冷却期：上一个会话未正常结束，请稍后 (约 30-60s) 重试")
    }
    if (data.code !== 0) {
      throw new Error(`连接失败: ${data.message || data.code}`)
    }

    await this.handleStartSuccess(data.data, { appKey, appSecret, code, appId })
  }

  async stop(): Promise<void> {
    if (!this.gameId && !this.roomId && !this.socketStatus.connected) return

    const gameId = this.gameId
    if (gameId) {
      try {
        await this.request("/v2/app/end", { game_id: gameId, app_id: this.appId })
      } catch (e) {
        console.error("[Bilibili/OpenPlatform] Failed to end game:", e)
      }
    }

    try {
      await this.config.set({ bilibili: { openPlatform: { gameId: "" } } })
    } catch (e) {
      console.error("[Bilibili/OpenPlatform] Failed to clear gameId in config:", e)
    }

    this.reset()
  }

  getStatus(): BilibiliStatus {
    return {
      source: this.type,
      connected: this.socketStatus.connected,
      roomId: this.socketStatus.roomId ?? this.roomId ?? undefined,
      gameId: this.gameId,
      error: this.socketStatus.error,
    }
  }

  private async request(path: string, params: Record<string, unknown> = {}): Promise<any> {
    const headers = signOpenPlatformRequest(params, this.credentials.appKey, this.credentials.appSecret)

    console.log(`[Bilibili/OpenPlatform] POST ${path}`)

    const resp = await fetch(`${BASE_URL}${path}`, {
      method: "POST",
      headers,
      body: JSON.stringify(params),
    })

    const data = await resp.json()
    console.log(`[Bilibili/OpenPlatform] Response ${path}: code=${data.code}`)
    return data
  }

  private async clearStaleGame(appId: number): Promise<void> {
    const staleGameId = this.config.bilibili.openPlatform.gameId
    if (!staleGameId) return

    console.log(`[Bilibili/OpenPlatform] Cleaning stale game from previous run: ${staleGameId}`)
    try {
      await this.request("/v2/app/end", { game_id: staleGameId, app_id: appId })
    } catch (e) {
      console.error("[Bilibili/OpenPlatform] Failed to end stale game:", e)
    }
    await this.config.set({ bilibili: { openPlatform: { gameId: "" } } })
  }

  private async handleStartSuccess(
    data: any,
    input: { appKey: string; appSecret: string; code: string; appId: number },
  ): Promise<void> {
    const { game_info, websocket_info } = data
    const auth = parseAuthBody(websocket_info?.auth_body)

    this.gameId = game_info.game_id
    this.roomId = typeof auth.roomid === "number" ? auth.roomid : null

    await this.config.set({
      bilibili: {
        source: this.type,
        openPlatform: { ...input, gameId: this.gameId ?? "" },
      },
    })

    this.httpHeartbeatTimer = setInterval(() => this.heartbeat(), 20000)
    this.socket.connect({
      label: "Bilibili/OpenPlatform",
      urls: websocket_info.wss_link,
      auth: {
        key: auth.key,
        group: auth.group,
        roomid: auth.roomid,
        protoover: auth.protoover ?? 2,
        uid: auth.uid ?? 0,
      },
      roomId: this.roomId,
      onMessage: (message) => this.handleMessage(message),
      onStatus: (status) => this.handleSocketStatus(status),
    })

    console.log(`[Bilibili/OpenPlatform] Started! Game ID: ${this.gameId}, Room: ${this.roomId}`)
  }

  private handleMessage(message: any): void {
    const gift = parseOpenPlatformGift(message)
    if (gift) this.eventBus.emit("gift", gift)
  }

  private handleSocketStatus(status: LiveSocketStatus): void {
    this.socketStatus = status
    this.eventBus.emit("bilibili:status", this.getStatus())
  }

  private async heartbeat(): Promise<void> {
    if (!this.gameId) return
    try {
      await this.request("/v2/app/heartbeat", { game_id: this.gameId })
    } catch (e) {
      console.error("[Bilibili/OpenPlatform] Heartbeat error:", e)
    }
  }

  private reset(): void {
    if (this.httpHeartbeatTimer) {
      clearInterval(this.httpHeartbeatTimer)
      this.httpHeartbeatTimer = null
    }
    this.gameId = null
    this.appId = 0
    this.roomId = null
    this.socketStatus = { connected: false }
    this.socket.disconnect()
    this.eventBus.emit("bilibili:status", this.getStatus())
  }
}

function parseAuthBody(authBody: unknown): any {
  if (typeof authBody !== "string" || authBody.length === 0) {
    throw new Error("auth_body 为空")
  }
  try {
    return JSON.parse(authBody)
  } catch {
    throw new Error("auth_body 格式错误")
  }
}
