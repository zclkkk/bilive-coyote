import { BILIBILI_SOURCE_TYPES, type AppConfig, type BilibiliSourceType, type GiftRule } from "./types"
import type { BilibiliStartInput } from "../bilibili/types"

export class ValidationError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "ValidationError"
  }
}

const COIN_TYPES = new Set(["gold", "silver", "all"])
const RULE_CHANNELS = new Set(["A", "B", "both"])
const STRENGTH_CHANNELS = new Set(["A", "B"])
const BILIBILI_SOURCES = new Set<string>(BILIBILI_SOURCE_TYPES)

export function validateConfig(value: unknown): AppConfig {
  const data = object(value, "config")
  const bilibili = object(data.bilibili, "bilibili")
  const openPlatform = object(bilibili.openPlatform, "bilibili.openPlatform")
  const broadcastCfg = object(bilibili.broadcast, "bilibili.broadcast")
  const coyote = object(data.coyote, "coyote")
  const server = object(data.server, "server")
  const safety = object(data.safety, "safety")

  return {
    bilibili: {
      source: enumString(bilibili.source, "bilibili.source", BILIBILI_SOURCES) as AppConfig["bilibili"]["source"],
      openPlatform: {
        appKey: string(openPlatform.appKey, "bilibili.openPlatform.appKey"),
        appSecret: string(openPlatform.appSecret, "bilibili.openPlatform.appSecret"),
        code: string(openPlatform.code, "bilibili.openPlatform.code"),
        appId: integer(openPlatform.appId, "bilibili.openPlatform.appId", 0),
      },
      broadcast: {
        roomId: integer(broadcastCfg.roomId, "bilibili.broadcast.roomId", 0),
      },
    },
    coyote: {
      wsPort: integer(coyote.wsPort, "coyote.wsPort", 1, 65535),
    },
    server: {
      httpPort: integer(server.httpPort, "server.httpPort", 1, 65535),
      host: nonEmptyString(server.host, "server.host"),
    },
    rules: validateRules(data.rules),
    safety: {
      limitA: integer(safety.limitA, "safety.limitA", 0, 200),
      limitB: integer(safety.limitB, "safety.limitB", 0, 200),
      decayEnabled: boolean(safety.decayEnabled, "safety.decayEnabled"),
      decayRate: integer(safety.decayRate, "safety.decayRate", 1, 200),
    },
  }
}

export function validateRules(value: unknown): GiftRule[] {
  if (!Array.isArray(value)) throw new ValidationError("rules must be an array")
  return value.map((item, index) => validateRule(item, `rules[${index}]`))
}

export function validateManualStrength(value: unknown): { channel: "A" | "B"; value: number } {
  const data = object(value, "body")
  const channel = enumString(data.channel, "channel", STRENGTH_CHANNELS) as "A" | "B"
  return {
    channel,
    value: integer(data.value, "value", 0, 200),
  }
}

export function validateBilibiliStart(value: unknown, defaultSource: BilibiliSourceType): BilibiliStartInput {
  const data = object(value, "body")
  const source = (optionalEnumString(data.source, "source", BILIBILI_SOURCES) ?? defaultSource) as BilibiliSourceType

  if (source === "open-platform") {
    return {
      source,
      code: optionalNonEmptyString(data.code, "code"),
      appId: optionalInteger(data.appId, "appId", 1),
      appKey: optionalNonEmptyString(data.appKey, "appKey"),
      appSecret: optionalNonEmptyString(data.appSecret, "appSecret"),
    }
  }

  return {
    source,
    roomId: optionalInteger(data.roomId, "roomId", 1),
  }
}

function validateRule(value: unknown, name: string): GiftRule {
  const data = object(value, name)
  const rule: GiftRule = {
    giftName: nonEmptyString(data.giftName, `${name}.giftName`),
    coinType: enumString(data.coinType, `${name}.coinType`, COIN_TYPES) as GiftRule["coinType"],
    channel: enumString(data.channel, `${name}.channel`, RULE_CHANNELS) as GiftRule["channel"],
    strengthAdd: integer(data.strengthAdd, `${name}.strengthAdd`, 1, 200),
    duration: integer(data.duration, `${name}.duration`, 1),
  }

  if (data.giftId !== undefined) {
    rule.giftId = integer(data.giftId, `${name}.giftId`, 1)
  }

  return rule
}

function object(value: unknown, name: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new ValidationError(`${name} must be an object`)
  }
  return value as Record<string, unknown>
}

function string(value: unknown, name: string): string {
  if (typeof value !== "string") throw new ValidationError(`${name} must be a string`)
  return value
}

function nonEmptyString(value: unknown, name: string): string {
  const result = string(value, name).trim()
  if (!result) throw new ValidationError(`${name} is required`)
  return result
}

function optionalNonEmptyString(value: unknown, name: string): string | undefined {
  if (value === undefined) return undefined
  return nonEmptyString(value, name)
}

function boolean(value: unknown, name: string): boolean {
  if (typeof value !== "boolean") throw new ValidationError(`${name} must be a boolean`)
  return value
}

function integer(value: unknown, name: string, min?: number, max?: number): number {
  if (typeof value !== "number" || !Number.isInteger(value)) throw new ValidationError(`${name} must be an integer`)
  if (min !== undefined && value < min) throw new ValidationError(`${name} must be >= ${min}`)
  if (max !== undefined && value > max) throw new ValidationError(`${name} must be <= ${max}`)
  return value
}

function optionalInteger(value: unknown, name: string, min?: number, max?: number): number | undefined {
  if (value === undefined) return undefined
  return integer(value, name, min, max)
}

function enumString(value: unknown, name: string, allowed: Set<string>): string {
  const result = string(value, name)
  if (!allowed.has(result)) throw new ValidationError(`${name} is invalid`)
  return result
}

function optionalEnumString(value: unknown, name: string, allowed: Set<string>): string | undefined {
  if (value === undefined) return undefined
  return enumString(value, name, allowed)
}
