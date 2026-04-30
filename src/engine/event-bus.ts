import { EventEmitter } from "events"

export interface GiftEvent {
  giftId: number
  giftName: string
  coinType: string
  totalCoin: number
  num: number
  uid: number
  uname: string
  timestamp: number
}

export interface StrengthChangeEvent {
  channel: "A" | "B"
  delta: number
  absolute?: number
  source: "gift" | "manual" | "decay" | "emergency"
  giftName?: string
  uname?: string
  duration?: number
}

export interface BilibiliStatusEvent {
  connected: boolean
  roomId?: number
  error?: string
}

export interface CoyoteStatusEvent {
  paired: boolean
  strengthA: number
  strengthB: number
  limitA: number
  limitB: number
  clientCount?: number
}

export type AppEvents = {
  gift: [GiftEvent]
  "strength:change": [StrengthChangeEvent]
  "bilibili:status": [BilibiliStatusEvent]
  "coyote:status": [CoyoteStatusEvent]
  "gift:log": [GiftEvent & { strengthDelta: string }]
}

export class EventBus extends EventEmitter {
  emit<K extends keyof AppEvents>(event: K, ...args: AppEvents[K]): boolean {
    return super.emit(event, ...args)
  }

  on<K extends keyof AppEvents>(event: K, listener: (...args: AppEvents[K]) => void): this {
    return super.on(event, listener)
  }
}
