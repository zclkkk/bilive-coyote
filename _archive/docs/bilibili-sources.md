# Bilibili 数据源

项目目前支持两个 Bilibili 礼物数据源：

| 数据源 | 需要配置 | 适合场景 |
|---|---|---|
| `broadcast` | 直播间房间号 | 本地自用、快速测试、不想配置开放平台 |
| `open-platform` | `AppKey`、`AppSecret`、主播身份码、`App ID` | 需要官方开放平台链路 |

两种数据源的后续礼物规则、强度计算和 Coyote 输出完全一致。实现细节见 [开发与架构](development.md#bilibili-source-抽象)。

## 怎么选

优先选择 `broadcast`：

- 只需要直播间房间号
- 不需要开放平台应用
- 不占用开放平台互动玩法会话

选择 `open-platform`：

- 已经有开放平台应用配置
- 需要官方开放平台链路
- 希望和开放平台互动玩法会话生命周期保持一致

## 开放平台

控制面板需要填写：

- `AppKey`
- `AppSecret`
- 主播身份码
- `App ID`

开放平台会把启动成功后的 `gameId` 写入运行时状态文件 `state.json`（和用户配置 `config.json` 分离）。进程异常退出后，下次启动会读取该值并先调用 `/v2/app/end` 清理残留会话。

当前只接收开放平台礼物事件 `LIVE_OPEN_PLATFORM_SEND_GIFT`。

## Broadcast

控制面板只需要填写直播间房间号，可以是短房间号。启动成功后，项目会保存解析后的长房间号。

当前只接收 Broadcast 礼物事件 `SEND_GIFT`。

## 配置片段

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
      "roomId": 6154037
    }
  }
}
```
