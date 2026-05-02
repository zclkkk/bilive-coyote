import type { GiftEvent } from "../../engine/event-bus";

const CMD_OPEN_PLATFORM_GIFT = "LIVE_OPEN_PLATFORM_SEND_GIFT";

interface OpenPlatformGiftMessage {
  cmd: string;
  data?: {
    gift_id?: number;
    giftId?: number;
    gift_name?: string;
    giftName?: string;
    paid?: boolean;
    price?: number;
    total_coin?: number;
    gift_num?: number;
    num?: number;
    uid?: number;
    uname?: string;
    username?: string;
    timestamp?: number;
  };
}

export function parseOpenPlatformGift(message: unknown): GiftEvent | null {
  const m = message as OpenPlatformGiftMessage | null;
  if (m?.cmd !== CMD_OPEN_PLATFORM_GIFT || !m.data) return null;

  const data = m.data;
  return {
    giftId: data.gift_id ?? data.giftId ?? 0,
    giftName: data.gift_name ?? data.giftName ?? "",
    coinType: data.paid === true ? "gold" : "silver",
    totalCoin: data.price ?? data.total_coin ?? 0,
    num: data.gift_num ?? data.num ?? 1,
    uid: data.uid ?? 0,
    uname: data.uname ?? data.username ?? "",
    timestamp: data.timestamp ?? Math.floor(Date.now() / 1000),
  };
}
