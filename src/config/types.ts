export interface GiftRule {
  giftName: string
  giftId?: number
  coinType: "gold" | "silver" | "all"
  channel: "A" | "B" | "both"
  strengthAdd: number
  duration: number
}

export const BILIBILI_SOURCE_TYPES = ["open-platform", "broadcast"] as const
export type BilibiliSourceType = typeof BILIBILI_SOURCE_TYPES[number]

export interface AppConfig {
  bilibili: {
    source: BilibiliSourceType
    openPlatform: {
      appKey: string
      appSecret: string
      code: string
      appId: number
      /** 上次 start 得到的 game_id；进程重启后用于清理残留会话，正常 end 后置空 */
      gameId?: string
    }
    broadcast: {
      roomId: number
    }
  }
  coyote: {
    wsPort: number
  }
  server: {
    httpPort: number
    host: string
  }
  rules: GiftRule[]
  safety: {
    limitA: number
    limitB: number
    decayEnabled: boolean
    decayRate: number
  }
}

export const DEFAULT_CONFIG: AppConfig = {
  bilibili: {
    source: "open-platform",
    openPlatform: {
      appKey: "",
      appSecret: "",
      code: "",
      appId: 0,
      gameId: "",
    },
    broadcast: {
      roomId: 0,
    },
  },
  coyote: {
    wsPort: 9999,
  },
  server: {
    httpPort: 3000,
    host: "0.0.0.0",
  },
  rules: [
    { giftName: "小心心", coinType: "silver", channel: "A", strengthAdd: 5, duration: 10 },
    { giftName: "辣条", coinType: "silver", channel: "B", strengthAdd: 3, duration: 5 },
  ],
  safety: {
    limitA: 80,
    limitB: 80,
    decayEnabled: true,
    decayRate: 2,
  },
}
