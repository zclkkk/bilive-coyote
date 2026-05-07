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

字段来源：

| 控制面板字段 | 获取位置 |
| --- | --- |
| `App ID` | [B 站直播开放平台](https://open-live.bilibili.com/open-manage) -> 自己的项目 -> `项目ID` |
| `AppKey` | 开放平台个人资料 -> `access_key_id` |
| `AppSecret` | 开放平台个人资料 -> `access_key_secred` |
| `主播身份码` | [B 站直播中心](https://link.bilibili.com/p/center/index) -> `我的直播间` -> `开播设置` -> `身份码` |

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

Broadcast 适合低配置使用。配置文件只保存直播间房间号：

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
  "roomId": 123456,
  "loginJson": "{\"cookie_info\":{\"cookies\":[...]}}"
}
```

`loginJson` 可选，内容是 BiliTV 登录 JSON 完整文本。它只用于本次 Broadcast 启动，不写入配置文件；如果游客态收不到礼物 `cmd`，再传入这个字段。

内部流程：

1. 获取直播间真实 room id。
2. 如果传入 `loginJson`，从 `cookie_info.cookies` 提取登录态 cookie。
3. 获取 WBI key 并签名 `getDanmuInfo`。
4. 使用 `uid`、`buvid` 和弹幕 key 鉴权连接 B 站直播 WebSocket。
5. 解析 `SEND_GIFT`。
6. 归一成 `GiftEvent`。

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
