import { ErrCode, type ErrCodeValue } from "./error-codes"

const MAX_MESSAGE_LENGTH = 1950
const WAVE_HEX_RE = /^[0-9a-fA-F]{16}$/

export interface CoyoteMessage {
  type: string | number
  clientId: string
  targetId: string
  message: string
  channel?: unknown
  strength?: unknown
  time?: unknown
}

export type ParseMessageResult =
  | { ok: true; message: CoyoteMessage }
  | { ok: false; code: ErrCodeValue }

export function parseMessage(data: string): ParseMessageResult {
  if (data.length > MAX_MESSAGE_LENGTH) return { ok: false, code: ErrCode.MESSAGE_TOO_LONG }

  try {
    const obj = JSON.parse(data)
    if (!obj || typeof obj !== "object" || Array.isArray(obj)) return { ok: false, code: ErrCode.INVALID_JSON }
    if (!hasValue(obj.type) || !hasValue(obj.clientId) || !hasValue(obj.targetId) || !hasValue(obj.message)) {
      return { ok: false, code: ErrCode.INVALID_JSON }
    }
    return {
      ok: true,
      message: {
        type: obj.type,
        clientId: String(obj.clientId),
        targetId: String(obj.targetId),
        message: String(obj.message),
        channel: obj.channel,
        strength: obj.strength,
        time: obj.time,
      },
    }
  } catch {
    return { ok: false, code: ErrCode.INVALID_JSON }
  }
}

export function buildMessage(type: string | number, clientId: string, targetId: string, message: string): string {
  return JSON.stringify({ type, clientId, targetId, message })
}

export function isValidChannel(value: unknown): value is "A" | "B" {
  return value === "A" || value === "B"
}

export function isValidFrontendChannel(value: unknown): value is 1 | 2 {
  return value === 1 || value === 2
}

export function isValidStrength(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 200
}

export function isValidDuration(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0
}

export function parsePulseHexArray(message: string): string[] | null {
  const colonIdx = message.indexOf(":")
  if (colonIdx < 0) return null
  try {
    const value = JSON.parse(message.substring(colonIdx + 1))
    if (!Array.isArray(value) || value.length === 0 || value.length > 100) return null
    if (!value.every(item => typeof item === "string" && WAVE_HEX_RE.test(item))) return null
    return value
  } catch {
    return null
  }
}

function buildStrengthMessage(channel: number, mode: number, value: number): string {
  return `strength-${channel}+${mode}+${value}`
}

export function parseStrengthFeedback(message: string): { a: number; b: number; limitA: number; limitB: number } | null {
  const match = message.match(/^strength-(\d+)\+(\d+)\+(\d+)\+(\d+)$/)
  if (!match) return null
  return {
    a: parseInt(match[1]),
    b: parseInt(match[2]),
    limitA: parseInt(match[3]),
    limitB: parseInt(match[4]),
  }
}

export function convertFrontendType(type: number, channel?: number, strength?: number): { message: string; channelNum: number } | null {
  const ch = channel ?? 1
  if (type === 1) {
    return { message: buildStrengthMessage(ch, 0, 1), channelNum: ch }
  }
  if (type === 2) {
    return { message: buildStrengthMessage(ch, 1, 1), channelNum: ch }
  }
  if (type === 3) {
    return { message: buildStrengthMessage(ch, 2, strength || 0), channelNum: ch }
  }
  return null
}

function hasValue(value: unknown): boolean {
  return value !== undefined && value !== null && value !== ""
}
