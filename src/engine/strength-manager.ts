import type { EventBus, StrengthChangeEvent } from "./event-bus"
import type { ConfigStore } from "../config/store"
import type { CoyoteServer } from "../coyote/server"

interface StrengthEntry {
  value: number
  baseline: number
  expiries: { until: number; delta: number }[]
}

export class StrengthManager {
  private config: ConfigStore
  private eventBus: EventBus
  private coyote: CoyoteServer
  private channels: Record<"A" | "B", StrengthEntry> = {
    A: { value: 0, baseline: 0, expiries: [] },
    B: { value: 0, baseline: 0, expiries: [] },
  }
  private appLimits = { a: 200, b: 200 }
  private decayTimer: ReturnType<typeof setInterval> | null = null

  constructor(config: ConfigStore, eventBus: EventBus, coyote: CoyoteServer) {
    this.config = config
    this.eventBus = eventBus
    this.coyote = coyote
    this.eventBus.on("strength:change", (e) => this.onStrengthChange(e))
    this.startDecayLoop()
  }

  updateAppLimits(limitA: number, limitB: number): void {
    this.appLimits.a = limitA
    this.appLimits.b = limitB
    this.enforceLimits()
  }

  applyAppFeedback(strengthA: number, strengthB: number): void {
    this.applyChannelFeedback("A", strengthA)
    this.applyChannelFeedback("B", strengthB)
  }

  resetLocal(): void {
    this.channels.A = { value: 0, baseline: 0, expiries: [] }
    this.channels.B = { value: 0, baseline: 0, expiries: [] }
    this.appLimits = { a: 200, b: 200 }
  }

  private getLimit(ch: "A" | "B"): number {
    const safety = this.config.safety
    const configLimit = ch === "A" ? safety.limitA : safety.limitB
    const appLimit = ch === "A" ? this.appLimits.a : this.appLimits.b
    return Math.min(configLimit, appLimit)
  }

  private onStrengthChange(e: StrengthChangeEvent): void {
    const ch = e.channel

    if (e.source === "emergency") {
      this.channels[ch].value = 0
      this.channels[ch].baseline = 0
      this.channels[ch].expiries = []
      this.coyote.sendStrength(ch, 2, 0)
      this.coyote.sendClear(ch)
      return
    }

    const limit = this.getLimit(ch)

    if (e.absolute !== undefined) {
      this.channels[ch].value = Math.min(e.absolute, limit)
      this.channels[ch].baseline = this.channels[ch].value
      this.channels[ch].expiries = []
    } else if (e.source === "gift") {
      const newValue = Math.min(this.channels[ch].value + e.delta, limit)
      const actualDelta = newValue - this.channels[ch].value
      if (actualDelta > 0) {
        this.channels[ch].value = newValue
        if (e.giftName) {
          this.channels[ch].expiries.push({
            until: Date.now() + (e.duration || 10) * 1000,
            delta: actualDelta,
          })
        }
      }
    } else if (e.source === "manual") {
      this.channels[ch].value = Math.min(Math.max(this.channels[ch].value + e.delta, 0), limit)
      this.channels[ch].baseline = this.channels[ch].value
      this.channels[ch].expiries = []
    } else if (e.source === "decay") {
      this.channels[ch].value = Math.max(this.channels[ch].value + e.delta, 0)
    }

    const val = this.channels[ch].value
    this.coyote.sendStrength(ch, 2, val)
  }

  enforceLimits(): void {
    for (const ch of ["A", "B"] as const) {
      const limit = this.getLimit(ch)
      if (this.channels[ch].value > limit) {
        this.channels[ch].value = limit
        this.channels[ch].baseline = limit
        this.channels[ch].expiries = []
        this.coyote.sendStrength(ch, 2, limit)
      }
    }
  }

  setManualStrength(channel: "A" | "B", value: number): void {
    this.onStrengthChange({ channel, delta: 0, absolute: value, source: "manual" })
  }

  emergencyStop(): void {
    this.onStrengthChange({ channel: "A", delta: 0, source: "emergency" })
    this.onStrengthChange({ channel: "B", delta: 0, source: "emergency" })
  }

  getStrength(channel: "A" | "B"): number {
    return this.channels[channel].value
  }

  getAppLimit(channel: "A" | "B"): number {
    return channel === "A" ? this.appLimits.a : this.appLimits.b
  }

  private applyChannelFeedback(channel: "A" | "B", appValue: number): void {
    const value = Math.min(appValue, this.getLimit(channel))
    const entry = this.channels[channel]

    if (entry.value !== value) {
      entry.value = value
      entry.baseline = value
      entry.expiries = []
    }

    if (appValue !== value) {
      this.coyote.sendStrength(channel, 2, value)
    }
  }

  private startDecayLoop(): void {
    this.decayTimer = setInterval(() => {
      const safety = this.config.safety
      if (!safety.decayEnabled) return

      for (const ch of ["A", "B"] as const) {
        const entry = this.channels[ch]
        const now = Date.now()

        entry.expiries = entry.expiries.filter(exp => exp.until > now)

        const activeDelta = entry.expiries.reduce((sum, exp) => sum + exp.delta, 0)
        const floor = entry.baseline + activeDelta

        if (entry.value > floor) {
          const decayDelta = Math.min(safety.decayRate, entry.value - floor)
          if (decayDelta > 0) {
            this.onStrengthChange({ channel: ch, delta: -decayDelta, source: "decay" })
          }
        }
      }
    }, 1000)
  }

  destroy(): void {
    if (this.decayTimer) clearInterval(this.decayTimer)
  }
}
