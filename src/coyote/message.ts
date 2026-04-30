export interface CoyoteMessage {
  type: string | number
  clientId: string
  targetId: string
  message: string
}

export function parseMessage(data: string): CoyoteMessage | null {
  try {
    const obj = JSON.parse(data)
    if (!obj || typeof obj !== "object") return null
    if (typeof obj.type === "undefined" || typeof obj.clientId === "undefined") return null
    return {
      type: obj.type,
      clientId: String(obj.clientId),
      targetId: String(obj.targetId ?? ""),
      message: String(obj.message ?? ""),
    }
  } catch {
    return null
  }
}

export function buildMessage(type: string | number, clientId: string, targetId: string, message: string): string {
  return JSON.stringify({ type, clientId, targetId, message })
}

export function isValidChannel(value: unknown): value is "A" | "B" {
  return value === "A" || value === "B"
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
  const ch = channel || 1
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
