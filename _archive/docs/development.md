# 开发与架构

## 项目结构

```txt
src/
  main.ts
  bilibili/
    service.ts
    types.ts
    live-socket.ts
    open-platform/
    broadcast/
  config/
    store.ts
    schema.ts
    types.ts
    runtime-state.ts
  coyote/
    server.ts
    message.ts
    error-codes.ts
  engine/
    event-bus.ts
    gift-mapper.ts
    strength-manager.ts
  server/
    main-server.ts
    router.ts
public/
  index.html
  css/
  js/
scripts/
  build.ts
```

## 数据流

```txt
BilibiliSource
  -> GiftEvent
  -> GiftMapper
  -> strength:change
  -> StrengthManager
  -> CoyoteServer
  -> DG-LAB APP

EventBus
  -> MainServer
  -> Panel WebSocket
```

## Bilibili source 抽象

`src/bilibili/types.ts` 定义数据源边界：

- `OpenPlatformStartInput`
- `BroadcastStartInput`
- `BilibiliStartInput`
- `BilibiliSource<T>`
- `BilibiliSources`

`BilibiliStartInput` 是 discriminated union。每个 source 只接收自己需要的启动参数，避免一个包含所有可选字段的参数对象在各层扩散。

`BilibiliService` 是门面：

1. 构造所有 source。
2. 根据 `config.bilibili.source` 选择初始 active source。
3. `start()` 时按 `input.source` 切换 active source。
4. `getStatus()` 只返回当前 active source 状态。

source 名称由 `BILIBILI_SOURCE_TYPES` 统一维护，schema 校验和类型定义共用同一份 source list。

## 共享 LiveSocket

`src/bilibili/live-socket.ts` 只负责 Bilibili 直播 WebSocket 传输层：

- 构造二进制认证包和心跳包
- 解析 Bilibili WS packet header
- 处理普通 JSON、Deflate、Brotli body
- 断线后按 URL 列表轮换重连
- 输出 `LiveSocketStatus`

`LiveSocketStatus` 不依赖 `EventBus`。source 收到 status 后再转换成 `BilibiliStatus` 并通过事件总线广播。

当前支持的 Bilibili WS op：

| op | 说明 |
|---|---|
| `2` | 心跳 |
| `3` | 心跳回应 |
| `5` | 消息 |
| `7` | 认证 |
| `8` | 认证成功 |

当前支持的 body protocol：

| protover | 说明 |
|---|---|
| `0` / `1` | 普通 body |
| `2` | Deflate |
| `3` | Brotli |

## 开放平台 source

目录：`src/bilibili/open-platform/`

文件职责：

| 文件 | 说明 |
|---|---|
| `source.ts` | 开放平台生命周期、HTTP API、WS 连接 |
| `signer.ts` | 开放平台请求签名 |
| `parser.ts` | 开放平台礼物事件解析 |
| `index.ts` | source 导出入口 |

启动路径：

1. 从 input 或 config 读取 `appKey`、`appSecret`、`code`、`appId`。
2. 如果 `state.json` 里有残留 `gameId`，先调用 `/v2/app/end` 清理。
3. 调用 `/v2/app/start`。
4. 解析返回的 `websocket_info.auth_body`。
5. 把新的 `gameId` 写入 `state.json`。
6. 启动 HTTP heartbeat。
7. 用 `BilibiliLiveSocket` 连接开放平台返回的 `wss_link`。

停止路径：

1. 如果当前有 `gameId`，调用 `/v2/app/end`。
2. 清空 `state.json` 中的 `gameId`。
3. 停止 HTTP heartbeat。
4. 断开 LiveSocket。

运行时状态（`state.json`）由 `RuntimeStateStore` 管理，和用户配置 (`config.json`) 解耦。可通过 `STATE_PATH` 环境变量指定其他路径。

礼物解析：

`parser.ts` 只处理 `LIVE_OPEN_PLATFORM_SEND_GIFT`。字段会归一成 `GiftEvent`：

```ts
{
  giftId,
  giftName,
  coinType,
  totalCoin,
  num,
  uid,
  uname,
  timestamp,
}
```

## Broadcast source

目录：`src/bilibili/broadcast/`

文件职责：

| 文件 | 说明 |
|---|---|
| `source.ts` | Broadcast WS 启动、状态、事件桥接 |
| `wbi.ts` | 房间号解析、WBI 签名、DanmuInfo 获取 |
| `parser.ts` | Broadcast 礼物事件解析 |
| `index.ts` | source 导出入口 |

启动路径：

1. 从 input 或 config 读取直播间房间号。
2. 调用 `mobileRoomInit` 把短房间号解析成长房间号。
3. 调用 `x/frontend/finger/spi` 获取 `buvid3`。
4. 调用 `x/web-interface/nav` 获取 WBI 图片 key。
5. 计算 mixin key，并签名 `getDanmuInfo` 请求。
6. 从 `getDanmuInfo` 获取 `token` 和完整 `host_list`。
7. 用长房间号发送 WS 认证包。
8. 把完整 host list 传给 `BilibiliLiveSocket`，用于重连轮换。
9. 保存解析后的长房间号到 config。

Broadcast WS 认证体：

```ts
{
  uid: 0,
  roomid: roomId,
  protover: 3,
  platform: "web",
  type: 2,
  key,
}
```

礼物解析：

`parser.ts` 只处理 `SEND_GIFT`，然后归一成同样的 `GiftEvent`。

## 礼物到强度

`GiftMapper` 监听 `gift`：

1. 按配置顺序查找第一条匹配规则。
2. 匹配成功则发出一个或多个 `strength:change`。
3. 无论是否匹配，都发出 `gift:log`，让控制面板能看到礼物流水。

规则匹配条件：

- `giftId` 设置时必须相等
- `giftName` 必须相等
- `coinType` 为 `all` 时忽略币种，否则必须相等

`StrengthManager` 负责：

- 当前强度状态
- APP 上限和本端 safety 上限合并
- 礼物持续时间
- 衰减
- 手动强度
- 紧急停止

## HTTP 和前端

`MainServer` 使用 Bun full-stack 路由：

```ts
import panel from "../../public/index.html"

Bun.serve({
  routes: {
    "/": panel,
  },
  fetch,
  websocket,
})
```

Bun 会处理 HTML import、JS/CSS 打包和静态资源路由。项目没有手写静态文件服务。

REST API 在 `src/server/router.ts`：

| 方法 | 路径 | 说明 |
|---|---|---|
| `POST` | `/api/bilibili/start` | 开始监听 |
| `POST` | `/api/bilibili/stop` | 停止监听 |
| `GET` | `/api/bilibili/status` | Bilibili 状态 |
| `GET` | `/api/coyote/status` | Coyote 状态 |
| `GET` | `/api/coyote/qrcode` | 配对二维码 |
| `POST` | `/api/coyote/strength` | 手动设置强度 |
| `POST` | `/api/coyote/emergency` | 紧急停止 |
| `GET` | `/api/config` | 获取配置 |
| `PUT` | `/api/config` | 更新配置 |
| `GET` | `/api/config/rules` | 获取礼物规则 |
| `PUT` | `/api/config/rules` | 更新礼物规则 |

控制面板 WebSocket 路径是 `/ws/panel`。

## 构建

`scripts/build.ts` 使用 Bun build API：

- `compile` 生成单文件可执行程序
- 默认构建当前平台
- 指定 target 时输出对应平台文件
- 构建前清理 `dist/`

前端通过 HTML import 内嵌进可执行程序，所以分发时只需要 `dist/` 里的可执行文件。
