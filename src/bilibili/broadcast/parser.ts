import type { GiftEvent } from "../../engine/event-bus"

interface BroadcastGiftMessage {
  cmd: string
  data?: {
    giftId: number
    giftName: string
    coin_type: string
    price: number
    num: number
    uid: number
    uname: string
    timestamp: number
  }
}

export function parseBroadcastGift(message: unknown): GiftEvent | null {
  const m = message as BroadcastGiftMessage | null
  if (m?.cmd !== "SEND_GIFT" || !m.data) return null

  const d = m.data
  return {
    giftId: d.giftId,
    giftName: d.giftName,
    coinType: d.coin_type,
    totalCoin: d.price,
    num: d.num,
    uid: d.uid,
    uname: d.uname,
    timestamp: d.timestamp,
  }
}
