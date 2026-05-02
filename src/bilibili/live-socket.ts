import { brotliDecompressSync, inflateSync } from "zlib";

const WS_OP_HEARTBEAT = 2;
const WS_OP_HEARTBEAT_REPLY = 3;
const WS_OP_MESSAGE = 5;
const WS_OP_AUTH = 7;
const WS_OP_CONNECT_SUCCESS = 8;

const WS_HEADER_LEN = 16;
const WS_BODY_PROTOCOL_VERSION_DEFLATE = 2;
const WS_BODY_PROTOCOL_VERSION_BROTLI = 3;

const HEARTBEAT_INTERVAL_MS = 20000;
const RECONNECT_BASE_MS = 3000;
const RECONNECT_MAX_MS = 60000;
const MAX_RECONNECT_ATTEMPTS = 5;

export interface LiveSocketStatus {
  connected: boolean;
  roomId?: number;
  error?: string;
}

interface LiveSocketOptions {
  label: string;
  urls: string[];
  auth: Record<string, unknown>;
  roomId?: number | null;
  onMessage: (message: unknown) => void;
  onStatus: (status: LiveSocketStatus) => void;
}

export class BilibiliLiveSocket {
  private ws: WebSocket | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private wssIndex = 0;
  private intentionalDisconnect = false;

  private options: LiveSocketOptions | null = null;
  private connected = false;
  private error: string | undefined;

  connect(options: LiveSocketOptions): void {
    this.disconnect();

    if (options.urls.length === 0) {
      this.options = options;
      this.setStatus(false, "wss_link 为空");
      return;
    }

    this.options = options;
    this.reconnectAttempts = 0;
    this.wssIndex = 0;
    this.intentionalDisconnect = false;
    this.setStatus(false);
    this.doConnect(options.urls[0]);
  }

  disconnect(): void {
    this.intentionalDisconnect = true;
    this.clearTimers();
    const ws = this.ws;
    this.ws = null;
    this.options = null;
    this.connected = false;
    this.error = undefined;
    if (ws) ws.close();
  }

  getStatus(): LiveSocketStatus {
    return {
      connected: this.connected,
      roomId: this.options?.roomId ?? undefined,
      error: this.error,
    };
  }

  private doConnect(url: string): void {
    const options = this.options;
    if (!options) return;

    console.log(`[${options.label}] Connecting to ${url}, room: ${options.roomId ?? "unknown"}`);
    this.ws = new WebSocket(url);
    this.ws.binaryType = "arraybuffer";

    this.ws.onopen = () => {
      console.log(`[${options.label}] Connected`);
      this.ws?.send(buildPacket(WS_OP_AUTH, JSON.stringify(options.auth)));
    };

    this.ws.onmessage = (event) => {
      const data = toUint8Array(event.data);
      if (data) this.handleData(data);
    };

    this.ws.onclose = () => {
      console.log(`[${options.label}] Disconnected`);
      this.clearTimers();
      if (!this.intentionalDisconnect) {
        this.setStatus(false, "弹幕连接断开，正在重连");
        this.tryReconnect();
      } else {
        this.setStatus(false);
      }
    };

    this.ws.onerror = (e) => {
      console.error(`[${options.label}] Error:`, e);
    };
  }

  private tryReconnect(): void {
    const options = this.options;
    if (!options) return;

    if (this.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
      console.error(`[${options.label}] Reconnect failed after ${MAX_RECONNECT_ATTEMPTS} attempts`);
      this.setStatus(false, `弹幕连接断开，已重试 ${MAX_RECONNECT_ATTEMPTS} 次仍失败，请手动重新连接`);
      return;
    }

    this.reconnectAttempts++;
    this.wssIndex = (this.wssIndex + 1) % options.urls.length;
    const url = options.urls[this.wssIndex];
    const delay = Math.min(RECONNECT_BASE_MS * 2 ** (this.reconnectAttempts - 1), RECONNECT_MAX_MS);

    console.log(
      `[${options.label}] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS})`,
    );
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.doConnect(url);
    }, delay);
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(buildPacket(WS_OP_HEARTBEAT, ""));
      }
    }, HEARTBEAT_INTERVAL_MS);
  }

  private handleData(buf: Uint8Array): void {
    let offset = 0;
    while (offset < buf.length) {
      if (buf.length - offset < WS_HEADER_LEN) break;

      const view = new DataView(buf.buffer, buf.byteOffset + offset);
      const totalLen = view.getUint32(0);
      const headerLen = view.getUint16(4);
      const protover = view.getUint16(6);
      const op = view.getUint32(8);

      if (totalLen > buf.length - offset) break;

      const body = buf.slice(offset + headerLen, offset + totalLen);

      switch (op) {
        case WS_OP_CONNECT_SUCCESS:
          console.log(`[${this.options?.label ?? "Bilibili"}] Auth success`);
          this.reconnectAttempts = 0;
          this.startHeartbeat();
          this.setStatus(true);
          break;

        case WS_OP_HEARTBEAT_REPLY:
          break;

        case WS_OP_MESSAGE:
          this.handleMessageBody(protover, body);
          break;
      }

      offset += totalLen;
    }
  }

  private handleMessageBody(protover: number, body: Uint8Array): void {
    if (protover === WS_BODY_PROTOCOL_VERSION_DEFLATE) {
      try {
        this.handleData(new Uint8Array(inflateSync(body)));
      } catch (e) {
        console.error(`[${this.options?.label ?? "Bilibili"}] Inflate error:`, e);
      }
      return;
    }

    if (protover === WS_BODY_PROTOCOL_VERSION_BROTLI) {
      try {
        this.handleData(new Uint8Array(brotliDecompressSync(body)));
      } catch (e) {
        console.error(`[${this.options?.label ?? "Bilibili"}] Brotli error:`, e);
      }
      return;
    }

    for (const message of parseJsonMessages(body)) {
      this.options?.onMessage(message);
    }
  }

  private clearTimers(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private setStatus(connected: boolean, error?: string): void {
    this.connected = connected;
    this.error = error;
    this.options?.onStatus(this.getStatus());
  }
}

function buildPacket(op: number, body: string): ArrayBuffer {
  const bodyBytes = new TextEncoder().encode(body);
  const totalLen = WS_HEADER_LEN + bodyBytes.length;
  const buf = new ArrayBuffer(totalLen);
  const view = new DataView(buf);

  view.setUint32(0, totalLen);
  view.setUint16(4, WS_HEADER_LEN);
  view.setUint16(6, 1);
  view.setUint32(8, op);
  view.setUint32(12, 1);
  new Uint8Array(buf, WS_HEADER_LEN).set(bodyBytes);

  return buf;
}

/**
 * 从 protover 0/1 的 body 里提取所有 JSON 消息。
 * B 站偶尔会在同一个 body 前后粘进控制字符或多段 JSON (弹幕协议 FAQ 提到过)，
 * 因此不能只做一次 JSON.parse，否则整帧会被吞掉 (包括 SEND_GIFT)。
 */
function parseJsonMessages(body: Uint8Array): unknown[] {
  const text = new TextDecoder().decode(body);
  const chunks = text
    .split(/[\x00-\x1f]+/)
    .map((item) => item.trim())
    .filter(Boolean);
  const messages: unknown[] = [];

  for (const chunk of chunks.length > 0 ? chunks : [text]) {
    const jsonStart = chunk.indexOf("{");
    if (jsonStart < 0) continue;
    try {
      const parsed = JSON.parse(chunk.slice(jsonStart));
      if (parsed && typeof parsed === "object") messages.push(parsed);
    } catch {
      // Bilibili occasionally mixes non-JSON fragments into compressed message bodies.
    }
  }

  return messages;
}

function toUint8Array(data: unknown): Uint8Array | null {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  return null;
}
