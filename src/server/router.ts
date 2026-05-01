import { ConfigStore } from "../config/store"
import { EventBus } from "../engine/event-bus"
import type { CoyoteServer } from "../coyote/server"
import type { StrengthManager } from "../engine/strength-manager"
import type { BilibiliClient } from "../bilibili/api"

export function createRouter(
  config: ConfigStore,
  eventBus: EventBus,
  coyote: CoyoteServer,
  strengthMgr: StrengthManager,
  bilibili: BilibiliClient,
) {
  const routes: Map<string, (req: Request, url: URL) => Promise<Response> | Response> = new Map()

  routes.set("GET /api/status", async () => {
    return Response.json({
      bilibili: bilibili.getStatus(),
      coyote: coyote.getStatus(),
      strength: {
        a: strengthMgr.getStrength("A"),
        b: strengthMgr.getStrength("B"),
        appLimitA: strengthMgr.getAppLimit("A"),
        appLimitB: strengthMgr.getAppLimit("B"),
      },
    })
  })

  routes.set("POST /api/bilibili/start", async (req) => {
    const body = await req.json() as { code?: string; appId?: number; appKey?: string; appSecret?: string }
    const code = body.code || config.bilibili.code
    const appId = body.appId || config.bilibili.appId
    if (!code || !appId) {
      return Response.json({ error: "code and appId required" }, { status: 400 })
    }
    if (body.appKey && body.appSecret) {
      bilibili.setCredentials(body.appKey, body.appSecret)
    }
    try {
      await bilibili.start(code, appId)
      return Response.json({ success: true })
    } catch (e: any) {
      return Response.json({ error: e.message }, { status: 500 })
    }
  })

  routes.set("POST /api/bilibili/stop", async () => {
    await bilibili.end()
    return Response.json({ success: true })
  })

  routes.set("GET /api/bilibili/status", async () => {
    return Response.json(bilibili.getStatus())
  })

  routes.set("GET /api/coyote/status", async () => {
    return Response.json(coyote.getStatus())
  })

  routes.set("GET /api/coyote/qrcode", async () => {
    const qr = await coyote.getQRCodeBase64()
    if (!qr) {
      return Response.json({ error: "No client connected" }, { status: 404 })
    }
    return Response.json({ qrcode: qr })
  })

  routes.set("POST /api/coyote/strength", async (req) => {
    const body = await req.json() as { channel: "A" | "B"; value: number }
    if (!body.channel || body.value === undefined) {
      return Response.json({ error: "channel and value required" }, { status: 400 })
    }
    strengthMgr.setManualStrength(body.channel, body.value)
    return Response.json({ success: true })
  })

  routes.set("POST /api/coyote/emergency", async () => {
    strengthMgr.emergencyStop()
    return Response.json({ success: true })
  })

  routes.set("GET /api/config", async () => {
    return Response.json(config.get())
  })

  routes.set("PUT /api/config", async (req) => {
    const body = await req.json()
    await config.set(body)
    if (body.safety) {
      strengthMgr.enforceLimits()
    }
    return Response.json({ success: true })
  })

  routes.set("GET /api/config/rules", async () => {
    return Response.json(config.rules)
  })

  routes.set("PUT /api/config/rules", async (req) => {
    const body = await req.json()
    await config.setRules(body)
    return Response.json({ success: true })
  })

  return routes
}

export function matchRoute(routes: Map<string, (req: Request, url: URL) => Promise<Response> | Response>, method: string, pathname: string): ((req: Request, url: URL) => Promise<Response> | Response) | null {
  const key = `${method} ${pathname}`
  return routes.get(key) ?? null
}
