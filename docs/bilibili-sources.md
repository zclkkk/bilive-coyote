# B 站数据源

项目支持两种礼物来源：

- 开放平台
- Broadcast 弹幕广播

两种来源最终都会归一成同样的 `GiftEvent`，后续礼物规则、强度计算和 Coyote 输出完全一致。

## 开放平台

开放平台适合正式接入，需要 B 站开放平台配置：

```json
{
  "bilibili": {
    "source": "open-platform",
    "openPlatform": {
      "appKey": "",
      "appSecret": "",
      "code": "",
      "appId": 0
    }
  }
}
```

启动 API 参数：

```json
{
  "source": "open-platform",
  "appKey": "...",
  "appSecret": "...",
  "code": "...",
  "appId": 123
}
```

内部流程：

1. 使用开放平台签名请求创建游戏。
2. 连接开放平台 WebSocket。
3. 解析礼物消息。
4. 归一成 `GiftEvent`。

## Broadcast

Broadcast 适合低配置使用，只需要直播间房间号：

```json
{
  "bilibili": {
    "source": "broadcast",
    "broadcast": {
      "roomId": 123456
    }
  }
}
```

启动 API 参数：

```json
{
  "source": "broadcast",
  "roomId": 123456
}
```

内部流程：

1. 获取直播间真实 room id。
2. 连接 B 站直播 WebSocket。
3. 解析 `SEND_GIFT`。
4. 归一成 `GiftEvent`。

## GiftEvent

归一后的礼物事件包含：

| 字段 | 说明 |
| --- | --- |
| `giftId` | 礼物 ID |
| `giftName` | 礼物名 |
| `coinType` | `gold` / `silver` 等 |
| `totalCoin` | 总瓜子数 |
| `num` | 数量 |
| `uid` | 用户 ID |
| `uname` | 用户名 |
| `timestamp` | Unix 秒时间戳 |

礼物规则只依赖这个归一结构。
