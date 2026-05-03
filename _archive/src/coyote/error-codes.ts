// DG-LAB SOCKET v2 协议错误码 (字符串)
// 参考: .temp/DG-LAB-OPENSOURCE/socket/v2/README.md
export const ErrCode = {
  SUCCESS: "200",
  PEER_DISCONNECTED: "209",
  INVALID_QR_CLIENT_ID: "210",
  NO_TARGET_ID: "211",
  ALREADY_BOUND: "400",
  TARGET_NOT_EXIST: "401",
  NOT_PAIRED: "402",
  INVALID_JSON: "403",
  PEER_OFFLINE: "404",
  MESSAGE_TOO_LONG: "405",
  CHANNEL_REQUIRED: "406",
  INTERNAL_ERROR: "500",
} as const;

export type ErrCodeValue = (typeof ErrCode)[keyof typeof ErrCode];
