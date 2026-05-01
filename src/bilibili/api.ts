import { signRequest } from "./signer"
import type { ConfigStore } from "../config/store"
import type { DanmakuWS } from "./danmaku-ws"

const BASE_URL = "https://live-open.biliapi.com"

export class BilibiliClient {
  private config: ConfigStore
  private danmaku: DanmakuWS | null = null
  private appKey = ""
  private appSecret = ""
  private appId = 0
  private gameId: string | null = null
  private httpHeartbeatTimer: ReturnType<typeof setInterval> | null = null
  private _connected = false
  private _roomId: number | null = null

  constructor(config: ConfigStore) {
    this.config = config
  }

  setDanmaku(danmaku: DanmakuWS): void {
    this.danmaku = danmaku
  }

  setCredentials(appKey: string, appSecret: string): void {
    this.appKey = appKey
    this.appSecret = appSecret
  }

  private async request(path: string, params: Record<string, unknown> = {}): Promise<any> {
    const key = this.appKey || this.config.bilibili.appKey
    const secret = this.appSecret || this.config.bilibili.appSecret

    const headers = signRequest(params, key, secret)

    console.log(`[Bilibili] POST ${path}`, params)

    const resp = await fetch(`${BASE_URL}${path}`, {
      method: "POST",
      headers,
      body: JSON.stringify(params),
    })

    const data = await resp.json()
    console.log(`[Bilibili] Response ${path}:`, JSON.stringify(data).slice(0, 200))
    return data
  }

  async start(code: string, appId: number): Promise<{ wssLinks: string[]; authBody: string; gameId: string }> {
    // 本进程内存中有残留 game——直接 end
    if (this.gameId) {
      await this.end()
    }

    // 上次进程退出未能正常 end (例如 SIGKILL、进程崩溃) —— 从 config 读出残留 gameId 并清理
    const staleGameId = this.config.bilibili.gameId
    if (staleGameId && staleGameId.length > 0) {
      console.log(`[Bilibili] Cleaning stale game from previous run: ${staleGameId}`)
      try {
        await this.request("/v2/app/end", { game_id: staleGameId, app_id: appId })
      } catch {}
      // 无论成败都清除，避免下次重复清理
      await this.config.set({ bilibili: { ...this.config.bilibili, gameId: "" } })
    }

    const data = await this.request("/v2/app/start", { code, app_id: appId })
    if (data.code === 7002) {
      console.log("[Bilibili] Room already has a game, trying to end and restart...")
      await this.request("/v2/app/end", { game_id: "", app_id: appId })
      const retryData = await this.request("/v2/app/start", { code, app_id: appId })
      if (retryData.code !== 0) {
        throw new Error(`重启失败: ${retryData.message || retryData.code}`)
      }
      return this.handleStartSuccess(retryData.data, code, appId)
    }
    if (data.code === 7001) {
      throw new Error(`请求冷却期：上一个会话未正常结束，请稍后 (约 30-60s) 重试`)
    }
    if (data.code !== 0) {
      throw new Error(`连接失败: ${data.message || data.code}`)
    }

    return this.handleStartSuccess(data.data, code, appId)
  }

  private handleStartSuccess(data: any, code: string, appId: number): { wssLinks: string[]; authBody: string; gameId: string } {
    const { game_info, websocket_info } = data
    this.gameId = game_info.game_id
    this.appId = appId
    this._connected = true
    this._roomId = parseRoomIdFromAuthBody(websocket_info?.auth_body)

    this.config.set({
      bilibili: { appKey: this.appKey, appSecret: this.appSecret, code, appId, gameId: this.gameId ?? "" },
    })

    this.httpHeartbeatTimer = setInterval(() => this.heartbeat(), 20000)

    if (this.danmaku) {
      this.danmaku.connect(websocket_info.wss_link, websocket_info.auth_body)
    }

    console.log(`[Bilibili] Started! Game ID: ${this.gameId}, Room: ${this._roomId}`)

    return {
      wssLinks: websocket_info.wss_link,
      authBody: websocket_info.auth_body,
      gameId: game_info.game_id,
    }
  }

  private async heartbeat(): Promise<void> {
    if (!this.gameId) return
    try {
      await this.request("/v2/app/heartbeat", { game_id: this.gameId })
    } catch (e) {
      console.error("[Bilibili] Heartbeat error:", e)
    }
  }

  async end(): Promise<void> {
    if (!this.gameId) return
    try {
      // /v2/app/end 需要 game_id + app_id，只传 game_id 会被拒为 4000 参数错误
      await this.request("/v2/app/end", { game_id: this.gameId, app_id: this.appId })
    } catch {}
    // 正常 end 后清除持久化的 gameId，避免下次启动重复清理
    try {
      await this.config.set({ bilibili: { ...this.config.bilibili, gameId: "" } })
    } catch {}
    this.stop()
  }

  stop(): void {
    if (this.httpHeartbeatTimer) {
      clearInterval(this.httpHeartbeatTimer)
      this.httpHeartbeatTimer = null
    }
    if (this.danmaku) {
      this.danmaku.disconnect()
    }
    this.gameId = null
    this.appId = 0
    this._connected = false
    this._roomId = null
  }

  getStatus() {
    return {
      connected: this._connected,
      roomId: this._roomId,
      gameId: this.gameId,
    }
  }
}

function parseRoomIdFromAuthBody(authBody: unknown): number | null {
  if (typeof authBody !== "string" || authBody.length === 0) return null
  try {
    const parsed = JSON.parse(authBody)
    return typeof parsed?.roomid === "number" ? parsed.roomid : null
  } catch {
    return null
  }
}
