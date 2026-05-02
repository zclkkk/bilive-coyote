import type { ConfigStore } from "../config/store";
import type { GiftRule } from "../config/types";
import type { EventBus, GiftEvent, StrengthChangeEvent } from "./event-bus";

export class GiftMapper {
  private eventBus: EventBus;
  private config: ConfigStore;

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config;
    this.eventBus = eventBus;
    this.eventBus.on("gift", (gift) => this.onGift(gift));
  }

  private onGift(gift: GiftEvent): void {
    const rule = this.config.rules.find((rule) => this.matchRule(rule, gift));
    const strengthDelta = rule ? this.applyRule(rule, gift) : "—";

    this.eventBus.emit("gift:log", {
      ...gift,
      strengthDelta,
    });
  }

  private matchRule(rule: GiftRule, gift: GiftEvent): boolean {
    if (rule.giftId !== undefined && gift.giftId !== rule.giftId) return false;
    if (gift.giftName !== rule.giftName) return false;
    if (rule.coinType !== "all" && rule.coinType !== gift.coinType) return false;
    return true;
  }

  private applyRule(rule: GiftRule, gift: GiftEvent): string {
    const channels: ("A" | "B")[] = rule.channel === "both" ? ["A", "B"] : [rule.channel];
    const delta = rule.strengthAdd * gift.num;

    for (const ch of channels) {
      const event: StrengthChangeEvent = {
        channel: ch,
        delta,
        source: "gift",
        giftName: gift.giftName,
        uname: gift.uname,
        duration: rule.duration,
      };
      this.eventBus.emit("strength:change", event);
    }

    return channels.map((ch) => `${ch}+${delta}`).join(" ");
  }
}
