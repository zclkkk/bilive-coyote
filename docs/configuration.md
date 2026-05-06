# 配置与礼物规则

## 配置结构

常用配置：

```json
{
  "bilibili": {
    "source": "broadcast",
    "openPlatform": {
      "appKey": "",
      "appSecret": "",
      "code": "",
      "appId": 0
    },
    "broadcast": {
      "roomId": 0
    }
  },
  "coyote": {
    "wsPort": 9999
  },
  "server": {
    "httpPort": 3000,
    "host": "0.0.0.0"
  },
  "rules": [
    {
      "giftName": "小心心",
      "coinType": "silver",
      "channel": "A",
      "strengthAdd": 5,
      "duration": 10
    }
  ],
  "safety": {
    "limitA": 80,
    "limitB": 80,
    "decayEnabled": true,
    "decayRate": 2
  }
}
```

## B 站配置

`bilibili.source` 可选：

- `open-platform`
- `broadcast`

开放平台配置：

| 字段 | 说明 |
| --- | --- |
| `appKey` | B 站开放平台 AppKey |
| `appSecret` | B 站开放平台 AppSecret |
| `code` | 主播身份码 |
| `appId` | 应用 ID |

Broadcast 配置：

| 字段 | 说明 |
| --- | --- |
| `roomId` | 直播间房间号 |

BiliTV 登录 JSON 是 Broadcast 启动参数 `loginJson`，不写入配置文件。

## Coyote 和服务配置

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `coyote.wsPort` | `9999` | DG-LAB APP Socket 端口 |
| `server.httpPort` | `3000` | 控制面板和 HTTP API 端口 |
| `server.host` | `0.0.0.0` | HTTP 与 Coyote WS 监听地址 |

## 安全配置

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `safety.limitA` | `80` | A 通道本端强度上限 |
| `safety.limitB` | `80` | B 通道本端强度上限 |
| `safety.decayEnabled` | `true` | 是否启用到期后衰减 |
| `safety.decayRate` | `2` | 每秒衰减强度 |

最终强度上限：

```txt
min(APP 软上限, safety.limitA/B)
```

## 礼物规则

规则按配置顺序匹配第一条。

匹配条件：

- `giftId` 存在时必须相等
- `giftName` 必须相等
- `coinType` 为 `all` 时忽略币种，否则必须相等

字段：

| 字段 | 说明 |
| --- | --- |
| `giftName` | 礼物名 |
| `giftId` | 可选，礼物 ID |
| `coinType` | `gold` / `silver` / `all` |
| `channel` | `A` / `B` / `both` |
| `strengthAdd` | 命中后增加的强度，0 到 200 |
| `duration` | 强度加成保留秒数 |
| `waveform` | 可选，波形 ID 或 `next` |

普通加强度：

```json
{
  "giftName": "小心心",
  "coinType": "silver",
  "channel": "A",
  "strengthAdd": 5,
  "duration": 10
}
```

加强度并切到指定波形：

```json
{
  "giftName": "辣条",
  "coinType": "silver",
  "channel": "B",
  "strengthAdd": 8,
  "duration": 10,
  "waveform": "tide"
}
```

只切到下一个波形：

```json
{
  "giftName": "换波形",
  "coinType": "all",
  "channel": "both",
  "strengthAdd": 0,
  "duration": 0,
  "waveform": "next"
}
```

规则约束：

- 没有 `waveform` 时，`strengthAdd` 必须大于 0，`duration` 必须大于 0。
- 有 `waveform` 时，允许 `strengthAdd: 0` 和 `duration: 0`，用于只切换波形。
- `strengthAdd > 0` 时，`duration` 仍然必须大于 0。

## 波形 ID

当前内置波形来自官方 `DG_WAVES_V2_V3_simple.js` 的 `expectedV3`。

| ID | 名称 |
| --- | --- |
| `breath` | 呼吸 |
| `tide` | 潮汐 |
| `combo` | 连击 |
| `fast_pinch` | 快速按捏 |
| `pinch_ramp` | 按捏渐强 |
| `heartbeat` | 心跳节奏 |
| `compress` | 压缩 |
| `pace` | 节奏步伐 |
| `grain` | 颗粒摩擦 |
| `gradient_bounce` | 渐变弹跳 |
| `ripple` | 波浪涟漪 |
| `rain` | 雨水冲刷 |
| `variable_tap` | 变速敲击 |
| `traffic_light` | 信号灯 |
| `tease_1` | 挑逗1 |
| `tease_2` | 挑逗2 |
