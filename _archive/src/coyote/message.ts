import { ErrCode, type ErrCodeValue } from "./error-codes";

const MAX_MESSAGE_LENGTH = 1950;

export interface CoyoteMessage {
  type: string | number;
  clientId: string;
  targetId: string;
  message: string;
}

export type ParseMessageResult = { ok: true; message: CoyoteMessage } | { ok: false; code: ErrCodeValue };

export function parseMessage(data: string): ParseMessageResult {
  if (data.length > MAX_MESSAGE_LENGTH) return { ok: false, code: ErrCode.MESSAGE_TOO_LONG };

  try {
    const obj = JSON.parse(data);
    if (!obj || typeof obj !== "object" || Array.isArray(obj)) return { ok: false, code: ErrCode.INVALID_JSON };
    if (!hasValue(obj.type) || !hasValue(obj.clientId) || !hasValue(obj.targetId) || !hasValue(obj.message)) {
      return { ok: false, code: ErrCode.INVALID_JSON };
    }
    return {
      ok: true,
      message: {
        type: obj.type,
        clientId: String(obj.clientId),
        targetId: String(obj.targetId),
        message: String(obj.message),
      },
    };
  } catch {
    return { ok: false, code: ErrCode.INVALID_JSON };
  }
}

export function buildMessage(type: string | number, clientId: string, targetId: string, message: string): string {
  return JSON.stringify({ type, clientId, targetId, message });
}

export function parseStrengthFeedback(
  message: string,
): { a: number; b: number; limitA: number; limitB: number } | null {
  const match = message.match(/^strength-(\d+)\+(\d+)\+(\d+)\+(\d+)$/);
  if (!match) return null;
  return {
    a: parseInt(match[1], 10),
    b: parseInt(match[2], 10),
    limitA: parseInt(match[3], 10),
    limitB: parseInt(match[4], 10),
  };
}

function hasValue(value: unknown): boolean {
  return value !== undefined && value !== null && value !== "";
}
