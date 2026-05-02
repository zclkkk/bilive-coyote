import { BilibiliLiveSocket, type LiveSocketStatus } from "../live-socket"
import { parseOpenPlatformGift } from "./parser"
import { signOpenPlatformRequest } from "./signer"
import type { BilibiliSource, BilibiliStatus, OpenPlatformStartInput } from "../types"
import type { ConfigStore } from "../../config/store"
import type { RuntimeStateStore } from "../../config/runtime-state"
import type { EventBus } from "../../engine/event-bus"

const BASE_URL = "https://live-open.biliapi.com"

interface OpenPlatformCredentials {
  appKey: string
  appSecret: string
}

interface OpenPlatformResponse<T = unknown> {
  code: number
  message?: string
  data: T
}

interface OpenPlatformStartData {
  game_info: { game_id: string }
  websocket_info: { wss_link: string[]; auth_body: string }
}

interface OpenPlatformAuthBody {
  key: string
  group?: string
  roomid?: number
  protoover?: number
  uid?: number
}

export class OpenPlatformSource implements BilibiliSource<"open-platform"> {
  readonly type = "open-platform" as const

  private config: ConfigStore
  private state: RuntimeStateStore
  private eventBus: EventBus
  private socket: BilibiliLiveSocket
  private credentials: OpenPlatformCredentials = { appKey: "", appSecret: "" }
  private appId = 0
  private gameId: string | null = null
  private httpHeartbeatTimer: ReturnType<typeof setInterval> | null = null
  private roomId: number | null = null
  private socketStatus: LiveSocketStatus = { connected: false }

  constructor(config: ConfigStore, state: RuntimeStateStore, eventBus: EventBus) {
    this.config = config
    this.state = state
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

    const resp = await this.request<OpenPlatformStartData>("/v2/app/start", { code, app_id: appId })
    if (resp.code === 7002) {
      throw new Error("直播间已有互动玩法会话，请先结束已有会话后重试")
    }
    if (resp.code === 7001) {
      throw new Error("请求冷却期：上一个会话未正常结束，请稍后 (约 30-60s) 重试")
    }
    if (resp.code !== 0) {
      throw new Error(`连接失败: ${resp.message || resp.code}`)
    }

    await this.handleStartSuccess(resp.data, { appKey, appSecret, code, appId })
  }

  async stop(): Promise<void> {
    const gameId = this.gameId
    const appId = this.appId
    this.reset()
    if (!gameId) return
    await this.endGame(gameId, appId, "Failed to end game")
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

  private async request<T = unknown>(path: string, params: Record<string, unknown> = {}): Promise<OpenPlatformResponse<T>> {
    const headers = signOpenPlatformRequest(params, this.credentials.appKey, this.credentials.appSecret)

    console.log(`[Bilibili/OpenPlatform] POST ${path}`)

    const resp = await fetch(`${BASE_URL}${path}`, {
      method: "POST",
      headers,
      body: JSON.stringify(params),
    })

    const data = (await resp.json()) as OpenPlatformResponse<T>
    console.log(`[Bilibili/OpenPlatform] Response ${path}: code=${data.code}`)
    return data
  }

  private async clearStaleGame(appId: number): Promise<void> {
    const staleGameId = this.state.openPlatformGameId
    if (!staleGameId) return

    console.log(`[Bilibili/OpenPlatform] Cleaning stale game from previous run: ${staleGameId}`)
    await this.endGame(staleGameId, appId, "Failed to end stale game")
  }

  /**
   * 调用 /v2/app/end 并在 **确认成功** (code === 0) 后清空 runtime state 里的 gameId。
   * 网络错误或非零 code 保留 gameId，以便下次启动继续清理。
   */
  private async endGame(gameId: string, appId: number, errLabel: string): Promise<void> {
    try {
      const resp = await this.request("/v2/app/end", { game_id: gameId, app_id: appId })
      if (resp.code !== 0) {
        console.error(`[Bilibili/OpenPlatform] ${errLabel}: code=${resp.code} message=${resp.message}`)
        return
      }
    } catch (e) {
      console.error(`[Bilibili/OpenPlatform] ${errLabel}:`, e)
      return
    }
    await this.state.setOpenPlatformGameId("")
  }

  private async handleStartSuccess(
    data: OpenPlatformStartData,
    input: { appKey: string; appSecret: string; code: string; appId: number },
  ): Promise<void> {
    const { game_info, websocket_info } = data
    const auth = parseAuthBody(websocket_info.auth_body)

    this.gameId = game_info.game_id
    this.roomId = typeof auth.roomid === "number" ? auth.roomid : null

    await this.state.setOpenPlatformGameId(this.gameId)
    await this.config.set({
      bilibili: {
        source: this.type,
        openPlatform: input,
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

  private handleMessage(message: unknown): void {
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

function parseAuthBody(authBody: unknown): OpenPlatformAuthBody {
  if (typeof authBody !== "string" || authBody.length === 0) {
    throw new Error("auth_body 为空")
  }
  try {
    return JSON.parse(authBody) as OpenPlatformAuthBody
  } catch {
    throw new Error("auth_body 格式错误")
  }
}
