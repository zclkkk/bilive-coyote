import { ConfigStore } from "./config/store"
import { BilibiliClient } from "./bilibili/api"
import { DanmakuWS } from "./bilibili/danmaku-ws"
import { CoyoteServer } from "./coyote/server"
import { EventBus } from "./engine/event-bus"
import { GiftMapper } from "./engine/gift-mapper"
import { StrengthManager } from "./engine/strength-manager"
import { MainServer } from "./server/main-server"

async function main() {
  console.log("[Bilive-Coyote] Starting...")

  const config = new ConfigStore()
  const eventBus = new EventBus()
  const bilibili = new BilibiliClient(config)
  const danmaku = new DanmakuWS(bilibili, eventBus)
  bilibili.setDanmaku(danmaku)
  const coyote = new CoyoteServer(config, eventBus)
  const giftMapper = new GiftMapper(config, eventBus)
  const strengthMgr = new StrengthManager(config, eventBus, coyote)
  const mainServer = new MainServer(config, eventBus, coyote, strengthMgr, bilibili, danmaku)

  eventBus.on("bilibili:status", (status) => {
    console.log("[Bilibili] Status:", status)
  })

  eventBus.on("coyote:status", (status) => {
    if (status.paired) {
      strengthMgr.updateAppLimits(status.limitA, status.limitB)
      strengthMgr.syncFromApp(status.strengthA, status.strengthB)
    } else {
      strengthMgr.resetLocal()
    }
  })

  eventBus.on("gift:log", (log) => {
    console.log(`[Gift] ${log.uname} sent ${log.giftName} x${log.num} (${log.strengthDelta})`)
  })

  await coyote.start()
  await mainServer.start()

  const { httpPort, host } = config.server
  const displayHost = host === "0.0.0.0" ? "localhost" : host
  console.log(`[Bilive-Coyote] Ready! Open http://${displayHost}:${httpPort}`)

  const shutdown = async () => {
    console.log("[Bilive-Coyote] Shutting down...")
    strengthMgr.destroy()
    coyote.stop()
    mainServer.stop()
    await bilibili.end()
    process.exit(0)
  }

  process.on("SIGINT", shutdown)
  process.on("SIGTERM", shutdown)
}

main().catch(console.error)
