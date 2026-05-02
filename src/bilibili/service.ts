import type { RuntimeStateStore } from "../config/runtime-state";
import type { ConfigStore } from "../config/store";
import type { EventBus } from "../engine/event-bus";
import { BroadcastSource } from "./broadcast";
import { OpenPlatformSource } from "./open-platform";
import type { BilibiliSources, BilibiliStartInput, BilibiliStatus } from "./types";

export class BilibiliService {
  private sources: BilibiliSources;
  private active: BilibiliSources[keyof BilibiliSources];

  constructor(config: ConfigStore, state: RuntimeStateStore, eventBus: EventBus) {
    const openPlatform = new OpenPlatformSource(config, state, eventBus);
    const broadcast = new BroadcastSource(config, eventBus);
    this.sources = {
      [openPlatform.type]: openPlatform,
      [broadcast.type]: broadcast,
    };
    this.active = this.sources[config.bilibili.source];
  }

  async start<T extends BilibiliStartInput["source"]>(
    input: Extract<BilibiliStartInput, { source: T }>,
  ): Promise<void> {
    if (this.active.type !== input.source) await this.active.stop();
    const next = this.sources[input.source] as BilibiliSources[T];
    this.active = next;
    await next.start(input);
  }

  async end(): Promise<void> {
    await this.active.stop();
  }

  getStatus(): BilibiliStatus {
    return this.active.getStatus();
  }
}
