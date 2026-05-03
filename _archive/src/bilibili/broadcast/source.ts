import type { ConfigStore } from "../../config/store";
import type { EventBus } from "../../engine/event-bus";
import { BilibiliLiveSocket, type LiveSocketStatus } from "../live-socket";
import type { BilibiliSource, BilibiliStatus, BroadcastStartInput } from "../types";
import { parseBroadcastGift } from "./parser";
import { fetchDanmuInfo } from "./wbi";

export class BroadcastSource implements BilibiliSource<"broadcast"> {
  readonly type = "broadcast" as const;

  private config: ConfigStore;
  private eventBus: EventBus;
  private socket: BilibiliLiveSocket;
  private roomId: number | null = null;
  private socketStatus: LiveSocketStatus = { connected: false };

  constructor(config: ConfigStore, eventBus: EventBus) {
    this.config = config;
    this.eventBus = eventBus;
    this.socket = new BilibiliLiveSocket();
  }

  async start(input: BroadcastStartInput): Promise<void> {
    if (this.socketStatus.connected) await this.stop();

    const requestedRoomId = input.roomId ?? this.config.bilibili.broadcast.roomId;
    if (!requestedRoomId) throw new Error("roomId required");

    const { key, urls, roomId } = await fetchDanmuInfo(requestedRoomId);
    this.roomId = roomId;

    this.socket.connect({
      label: "Bilibili/Broadcast",
      urls,
      auth: {
        uid: 0,
        roomid: roomId,
        protover: 3,
        platform: "web",
        type: 2,
        key,
      },
      roomId: roomId,
      onMessage: (message) => this.handleMessage(message),
      onStatus: (status) => this.handleSocketStatus(status),
    });

    console.log(`[Bilibili/Broadcast] Started! Room: ${roomId}`);

    await this.config.set({
      bilibili: {
        source: this.type,
        broadcast: { roomId },
      },
    });
  }

  async stop(): Promise<void> {
    this.reset();
  }

  getStatus(): BilibiliStatus {
    return {
      source: this.type,
      connected: this.socketStatus.connected,
      roomId: this.socketStatus.roomId ?? this.roomId ?? undefined,
      error: this.socketStatus.error,
    };
  }

  private handleSocketStatus(status: LiveSocketStatus): void {
    this.socketStatus = status;
    this.eventBus.emit("bilibili:status", this.getStatus());
  }

  private handleMessage(message: unknown): void {
    const gift = parseBroadcastGift(message);
    if (gift) this.eventBus.emit("gift", gift);
  }

  private reset(): void {
    this.roomId = null;
    this.socketStatus = { connected: false };
    this.socket.disconnect();
    this.eventBus.emit("bilibili:status", this.getStatus());
  }
}
