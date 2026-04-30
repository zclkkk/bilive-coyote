import type { EventBus, GiftEvent, StrengthChangeEvent } from "./event-bus"
import type { ConfigStore } from "../config/store"
import type { GiftRule } from "../config/types"

export class GiftMapper {
  private eventBus: EventBus
  private config: ConfigStore

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config
    this.eventBus = eventBus
    this.eventBus.on("gift", (gift) => this.onGift(gift))
  }

  private onGift(gift: GiftEvent): void {
    const rules = this.config.rules
    // first-match: 找到第一条匹配规则就停止，避免同一礼物被多条规则重复加强度
    for (const rule of rules) {
      if (this.matchRule(rule, gift)) {
        this.applyRule(rule, gift)
        return
      }
    }
  }

  private matchRule(rule: GiftRule, gift: GiftEvent): boolean {
    if (rule.giftId && gift.giftId !== rule.giftId) return false
    if (rule.giftName && gift.giftName !== rule.giftName) return false
    if (rule.coinType !== "all" && rule.coinType !== gift.coinType) return false
    return true
  }

  private applyRule(rule: GiftRule, gift: GiftEvent): void {
    const channels: ("A" | "B")[] = rule.channel === "both" ? ["A", "B"] : [rule.channel]
    const delta = rule.strengthAdd * gift.num

    for (const ch of channels) {
      const event: StrengthChangeEvent = {
        channel: ch,
        delta,
        source: "gift",
        giftName: gift.giftName,
        uname: gift.uname,
        duration: rule.duration,
      }
      this.eventBus.emit("strength:change", event)
    }

    this.eventBus.emit("gift:log", {
      ...gift,
      strengthDelta: channels.map(ch => `${ch}+${delta}`).join(" "),
    })
  }
}
