import type { BilibiliSourceType } from "../config/types"

export interface OpenPlatformStartInput {
  source: "open-platform"
  appKey?: string
  appSecret?: string
  code?: string
  appId?: number
}

export interface BroadcastStartInput {
  source: "broadcast"
  roomId?: number
}

export type BilibiliStartInput = OpenPlatformStartInput | BroadcastStartInput

export interface BilibiliStatus {
  source: BilibiliSourceType
  connected: boolean
  roomId?: number
  gameId?: string | null
  error?: string
}

export interface BilibiliSource<T extends BilibiliSourceType = BilibiliSourceType> {
  readonly type: T
  start(input: Extract<BilibiliStartInput, { source: T }>): Promise<void>
  stop(): Promise<void>
  getStatus(): BilibiliStatus
}

export type BilibiliSources = {
  [T in BilibiliSourceType]: BilibiliSource<T>
}
