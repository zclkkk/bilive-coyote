import type { BilibiliSourceType } from "../config/types"
import type { BilibiliStatusEvent } from "../engine/event-bus"

export interface BilibiliStartInput {
  source?: BilibiliSourceType
  appKey?: string
  appSecret?: string
  code?: string
  appId?: number
  roomId?: number
}

export interface BilibiliStatus extends BilibiliStatusEvent {
  source: BilibiliSourceType
  gameId?: string | null
}

export interface BilibiliSource {
  readonly type: BilibiliSourceType
  start(input: BilibiliStartInput): Promise<void>
  stop(): Promise<void>
  getStatus(): BilibiliStatus
}
