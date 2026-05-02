import type { GiftEvent } from "../../engine/event-bus"

export function parseBroadcastGift(message: any): GiftEvent | null {
  if (message?.cmd !== "SEND_GIFT" || !message.data) return null

  const d = message.data
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
