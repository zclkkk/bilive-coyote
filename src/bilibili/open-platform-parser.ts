import type { GiftEvent } from "../engine/event-bus"

const CMD_OPEN_PLATFORM_GIFT = "LIVE_OPEN_PLATFORM_SEND_GIFT"

export function parseOpenPlatformGift(message: any): GiftEvent | null {
  if (message?.cmd !== CMD_OPEN_PLATFORM_GIFT || !message.data) return null

  const data = message.data
  return {
    giftId: data.gift_id ?? data.giftId ?? 0,
    giftName: data.gift_name ?? data.giftName ?? "",
    coinType: data.paid === true ? "gold" : "silver",
    totalCoin: data.price ?? data.total_coin ?? 0,
    num: data.gift_num ?? data.num ?? 1,
    uid: data.uid ?? 0,
    uname: data.uname ?? data.username ?? "",
    timestamp: data.timestamp ?? Math.floor(Date.now() / 1000),
  }
}
