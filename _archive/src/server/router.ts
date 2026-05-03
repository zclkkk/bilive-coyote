import type { BilibiliService } from "../bilibili/service";
import { validateBilibiliStart, validateManualStrength } from "../config/schema";
import type { ConfigStore } from "../config/store";
import type { CoyoteServer } from "../coyote/server";
import type { StrengthManager } from "../engine/strength-manager";

export function createRouter(
  config: ConfigStore,
  coyote: CoyoteServer,
  strengthMgr: StrengthManager,
  bilibili: BilibiliService,
) {
  const routes: Map<string, (req: Request, url: URL) => Promise<Response> | Response> = new Map();

  routes.set("GET /api/status", async () => {
    return Response.json({
      bilibili: bilibili.getStatus(),
      coyote: coyote.getStatus(),
      strength: {
        a: strengthMgr.getStrength("A"),
        b: strengthMgr.getStrength("B"),
        appLimitA: strengthMgr.getAppLimit("A"),
        appLimitB: strengthMgr.getAppLimit("B"),
        effectiveLimitA: strengthMgr.getLimit("A"),
        effectiveLimitB: strengthMgr.getLimit("B"),
      },
    });
  });

  routes.set("POST /api/bilibili/start", async (req) => {
    const input = validateBilibiliStart(await req.json(), config.bilibili.source);
    await bilibili.start(input);
    return Response.json({ success: true });
  });

  routes.set("POST /api/bilibili/stop", async () => {
    await bilibili.end();
    return Response.json({ success: true });
  });

  routes.set("GET /api/bilibili/status", async () => {
    return Response.json(bilibili.getStatus());
  });

  routes.set("GET /api/coyote/status", async () => {
    return Response.json(coyote.getStatus());
  });

  routes.set("GET /api/coyote/qrcode", async () => {
    const qr = await coyote.getQRCodeBase64();
    if (!qr) {
      return Response.json({ error: "QR code unavailable" }, { status: 404 });
    }
    return Response.json({ qrcode: qr });
  });

  routes.set("POST /api/coyote/strength", async (req) => {
    const body = validateManualStrength(await req.json());
    strengthMgr.setManualStrength(body.channel, body.value);
    return Response.json({ success: true });
  });

  routes.set("POST /api/coyote/emergency", async () => {
    strengthMgr.emergencyStop();
    return Response.json({ success: true });
  });

  routes.set("GET /api/config", async () => {
    return Response.json(config.get());
  });

  routes.set("PUT /api/config", async (req) => {
    await config.set(await req.json());
    strengthMgr.enforceLimits();
    return Response.json({ success: true });
  });

  routes.set("GET /api/config/rules", async () => {
    return Response.json(config.rules);
  });

  routes.set("PUT /api/config/rules", async (req) => {
    const body = await req.json();
    await config.setRules(body);
    return Response.json({ success: true });
  });

  return routes;
}

export function matchRoute(
  routes: Map<string, (req: Request, url: URL) => Promise<Response> | Response>,
  method: string,
  pathname: string,
): ((req: Request, url: URL) => Promise<Response> | Response) | null {
  const key = `${method} ${pathname}`;
  return routes.get(key) ?? null;
}
