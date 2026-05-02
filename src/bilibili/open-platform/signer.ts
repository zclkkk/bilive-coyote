import { createHash, createHmac } from "crypto";

export function signOpenPlatformRequest(
  params: Record<string, unknown>,
  appKey: string,
  appSecret: string,
): Record<string, string> {
  const timestamp = Math.floor(Date.now() / 1000);
  const nonce = Math.floor(Math.random() * 100000) + timestamp;

  const headers: Record<string, string> = {
    "x-bili-accesskeyid": appKey,
    "x-bili-content-md5": createHash("md5").update(JSON.stringify(params)).digest("hex"),
    "x-bili-signature-method": "HMAC-SHA256",
    "x-bili-signature-nonce": String(nonce),
    "x-bili-signature-version": "1.0",
    "x-bili-timestamp": String(timestamp),
  };

  const data = Object.entries(headers)
    .map(([key, value]) => `${key}:${value}`)
    .join("\n");
  const signature = createHmac("sha256", appSecret).update(data).digest("hex");

  return {
    "Content-Type": "application/json",
    Accept: "application/json",
    ...headers,
    Authorization: signature,
  };
}
