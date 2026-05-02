import { OpenPlatformSource } from "./open-platform"
import { BroadcastSource } from "./broadcast"
import type { BilibiliSources, BilibiliStartInput, BilibiliStatus } from "./types"
import type { ConfigStore } from "../config/store"
import type { EventBus } from "../engine/event-bus"

export class BilibiliService {
  private sources: BilibiliSources
  private active: BilibiliSources[keyof BilibiliSources]

  constructor(config: ConfigStore, eventBus: EventBus) {
    const openPlatform = new OpenPlatformSource(config, eventBus)
    const broadcast = new BroadcastSource(config, eventBus)
    this.sources = {
      [openPlatform.type]: openPlatform,
      [broadcast.type]: broadcast,
    }
    this.active = this.sources[config.bilibili.source]
  }

  async start(input: BilibiliStartInput): Promise<void> {
    const next = this.sources[input.source]

    if (next !== this.active) await this.active.stop()
    this.active = next
    if (input.source === "open-platform") {
      await this.sources["open-platform"].start(input)
    } else {
      await this.sources.broadcast.start(input)
    }
  }

  async end(): Promise<void> {
    await this.active.stop()
  }

  getStatus(): BilibiliStatus {
    return this.active.getStatus()
  }
}
