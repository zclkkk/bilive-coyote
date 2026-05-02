import { OpenPlatformSource } from "./open-platform"
import type { BilibiliSource, BilibiliStartInput, BilibiliStatus } from "./types"
import type { ConfigStore } from "../config/store"
import type { BilibiliSourceType } from "../config/types"
import type { EventBus } from "../engine/event-bus"

export class BilibiliService {
  private config: ConfigStore
  private sources: Partial<Record<BilibiliSourceType, BilibiliSource>>
  private active: BilibiliSource

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config
    const openPlatform = new OpenPlatformSource(config, eventBus)
    this.sources = {
      "open-platform": openPlatform,
    }
    this.active = openPlatform
  }

  async start(input: BilibiliStartInput): Promise<void> {
    const source = input.source ?? this.config.bilibili.source
    const next = this.getSource(source)

    if (next !== this.active) await this.active.stop()
    this.active = next
    await next.start(input)
  }

  async end(): Promise<void> {
    await this.active.stop()
  }

  getStatus(): BilibiliStatus {
    return this.active.getStatus()
  }

  private getSource(source: BilibiliSourceType): BilibiliSource {
    const implementation = this.sources[source]
    if (implementation) return implementation
    throw new Error("观众端 WS 模式尚未实现")
  }
}
