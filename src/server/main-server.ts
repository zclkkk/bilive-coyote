import type { Server, ServerWebSocket } from "bun";
import panel from "../../public/index.html";
import type { BilibiliService } from "../bilibili/service";
import { ValidationError } from "../config/schema";
import type { ConfigStore } from "../config/store";
import type { CoyoteServer } from "../coyote/server";
import type { EventBus } from "../engine/event-bus";
import type { StrengthManager } from "../engine/strength-manager";
import { createRouter, matchRoute } from "./router";

type PanelWs = ServerWebSocket<{ channel: "panel" }>;

export class MainServer {
  private config: ConfigStore;
  private eventBus: EventBus;
  private strengthMgr: StrengthManager;
  private routes: Map<string, (req: Request, url: URL) => Promise<Response> | Response>;
  private panelClients: Set<PanelWs> = new Set();
  private server: Server<{ channel: "panel" }> | null = null;

  constructor(
    config: ConfigStore,
    eventBus: EventBus,
    coyote: CoyoteServer,
    strengthMgr: StrengthManager,
    bilibili: BilibiliService,
  ) {
    this.config = config;
    this.eventBus = eventBus;
    this.strengthMgr = strengthMgr;
    this.routes = createRouter(config, coyote, strengthMgr, bilibili);
    this.setupEventForwarding();
  }

  async start(): Promise<void> {
    const { httpPort, host } = this.config.server;

    this.server = Bun.serve<{ channel: "panel" }>({
      port: httpPort,
      hostname: host,
      routes: {
        "/": panel,
      },
      fetch: (req, server) => this.handleRequest(req, server),
      websocket: {
        open: (ws) => this.onWsOpen(ws),
        message: () => {},
        close: (ws) => this.onWsClose(ws),
      },
    });

    console.log(`[Server] HTTP + WS started on http://${host}:${httpPort}`);
  }

  private async handleRequest(req: Request, server: Server<{ channel: "panel" }>): Promise<Response> {
    const url = new URL(req.url);

    if (url.pathname === "/ws/panel") {
      if (server.upgrade(req, { data: { channel: "panel" } })) return new Response(null);
      return new Response("WS upgrade failed", { status: 500 });
    }

    if (url.pathname.startsWith("/api/")) {
      const handler = matchRoute(this.routes, req.method, url.pathname);
      if (handler) {
        try {
          return await handler(req, url);
        } catch (e: any) {
          if (e instanceof ValidationError) {
            return Response.json({ error: e.message }, { status: 400 });
          }
          return Response.json({ error: e.message }, { status: 500 });
        }
      }
      return Response.json({ error: "Not found" }, { status: 404 });
    }

    return new Response("Not found", { status: 404 });
  }

  private onWsOpen(ws: PanelWs): void {
    if (ws.data.channel === "panel") this.panelClients.add(ws);
  }

  private onWsClose(ws: PanelWs): void {
    this.panelClients.delete(ws);
  }

  private setupEventForwarding(): void {
    this.eventBus.on("gift:log", (data) => {
      this.broadcast({ type: "gift", data });
    });

    this.eventBus.on("strength:change", (data) => {
      this.broadcast({
        type: "strength",
        data: {
          channel: data.channel,
          value: this.strengthMgr.getStrength(data.channel),
          source: data.source,
        },
      });
    });

    this.eventBus.on("bilibili:status", (data) => {
      this.broadcast({ type: "bilibili:status", data });
    });

    this.eventBus.on("coyote:status", (data) => {
      this.broadcast({
        type: "coyote:status",
        data: {
          ...data,
          effectiveLimitA: this.strengthMgr.getLimit("A"),
          effectiveLimitB: this.strengthMgr.getLimit("B"),
        },
      });
    });
  }

  private broadcast(data: unknown): void {
    const msg = JSON.stringify(data);
    for (const ws of this.panelClients) ws.send(msg);
  }

  stop(): void {
    if (this.server) this.server.stop();
  }
}
