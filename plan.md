# Bilive-Coyote → Rust 迁移计划

## 技术决策

| 决策 | 选择 |
|------|------|
| 异步运行时 | `tokio` |
| Web 框架 | `axum` |
| 架构模式 | AppController + Command Channel + View Broadcast |
| 配置管理 | `Arc<RwLock<Config>>` |
| 错误处理 | `thiserror`（库层） + `anyhow`（应用层） |
| 前端部署 | `rust-embed` 嵌入二进制 |
| 日志 | `tracing` |
| 二进制协议解析 | `bytes` crate |
| WebSocket Client | `tokio-tungstenite` |

## 数据流

```
HTTP API ───────────────┐
BilibiliClient ─────────┼──→ mpsc<AppCommand> ──→ AppController
CoyoteHub feedback ─────┘                              │
                                                       ├── broadcast<AppView> ──→ Panel WS
                                                       ├── mpsc<DeviceCommand> ──→ CoyoteHub
                                                       └── ConfigStore
```

## 完成条件

每个 Phase 完成时必须满足以下条件之一：
- 该 Phase 描述的所有测试通过
- 该 Phase 描述的手动验证步骤通过
- 后续 Phase 不依赖该 Phase 中标记为可延后的部分

---

## Phase 1 — 项目骨架

**目标：** 可编译、可启动的空应用，验证依赖和编译设置。

**内容：**
- 初始化 Cargo 项目
- 配置 `Cargo.toml`，锁定核心依赖版本
- 建立目录结构
- 设置 `[profile.release]` 编译优化参数
- 配置 `tracing-subscriber` 日志输出（支持 `RUST_LOG` 环境变量）
- 编写 `main.rs` 骨架：启动 tokio runtime，挂载 SIGINT/SIGTERM 信号处理，优雅退出占位

**新增文件：**
```
Cargo.toml
src/main.rs
src/lib.rs
```

**完成条件：** `cargo build --release` 通过

---

## Phase 2 — 核心类型 + Config + 最小 AppController

**目标：** 定义所有数据结构，实现配置持久化，Controller 的最小可运行版本。

**内容：**
- 定义 `Config` / `GiftRule` / `SafetyConfig` 等核心 struct（`serde` 序列化/反序列化）
- 实现 `ConfigStore`：从 `config.json` 加载，`Arc<RwLock<Config>>` 持有，写操作后写回文件，文件不存在时回退到默认配置
- 定义 `AppCommand` 枚举（覆盖所有系统事件和用户操作）
- 定义 `AppView` 枚举（纯 UI 输出，推给 Panel/Overlay）
- 定义 `DeviceCommand` 枚举（硬件指令，不混入 AppView）
- 定义 `AppError` 枚举（`thiserror`），涵盖：B站 API 错误、Coyote 协议错误、IO 错误、配置错误
- 实现最小 `AppController`：处理 `Ping` / `Shutdown` / `SetStrength`（mock 模式）

**新增文件：**
```
src/config.rs          # Config / GiftRule / SafetyConfig + ConfigStore
src/command.rs         # AppCommand 枚举
src/view.rs            # AppView 枚举
src/device.rs          # DeviceCommand 枚举
src/error.rs           # AppError 枚举 (thiserror)
src/engine/
src/engine/mod.rs
src/engine/controller.rs  # struct AppController + 主循环 run()
```

**完成条件：**
- `ConfigStore` 加载/保存/合并的单测通过
- Controller 能处理 `Ping` / `Shutdown` / `SetStrength`，单测通过
- `cargo test` 全部通过

---

## Phase 3 — HTTP API + Panel WS + 静态文件

**目标：** 先把前端通路跑起来，打通"浏览器按钮 → HTTP API → AppCommand → Controller → AppView → Panel WS"。

**内容：**
- 注册所有 `/api/*` 路由到 axum Router
- 每个 handler：从 `State` 提取 `Command channel sender` 和 `Config`，解析请求体，推入 Command channel 或直接读 config
- 标准化错误响应格式（与现有前端期望的 `{ error: "..." }` 兼容）
- 实现 `/ws/panel` WebSocket handler：`subscribe` View broadcast channel，收到 `AppView` 后序列化 JSON 推送（每个连接独立 subscribe，不维护全局客户端集合）
- 用 `rust-embed` 在编译时将 `public/` 目录内容嵌入二进制
- 实现静态文件服务：正确 MIME type、未知路径 fallback 到 `index.html`

**新增文件：**
```
src/api/
src/api/mod.rs
src/api/router.rs       # axum::Router 组装
src/api/bilibili.rs     # /api/bilibili/* handlers
src/api/coyote.rs       # /api/coyote/* handlers
src/api/config.rs       # /api/config/* handlers
src/api/panel_ws.rs     # /ws/panel WebSocket handler (broadcast subscribe)
src/static_files.rs     # rust-embed + 静态文件服务 + MIME 映射
```

**完成条件：**
- 浏览器打开控制面板，所有静态资源正常加载
- Panel WS 能收到测试 AppView 并更新 UI
- GET/PUT `/api/config` 等端点可正常调用

---

## Phase 4 — Strength Engine

**目标：** 实现核心业务逻辑——礼物映射、强度管理、安全限制、衰减，不依赖真实 B站或 Coyote。

**内容：**
- 礼物规则匹配（first-match 策略，根据 `giftName` / `giftId` / `coinType` 匹配）
- 手动强度设置
- 紧急停止（A/B 通道归零 + 清空到期列表）
- 双重安全上限 = `min(本端配置限制, APP 端上限)`
- 衰减循环：每秒检查活跃到期项是否过期，衰减到 floor（baseline + 活跃 delta 之和）
- APP feedback 回声过滤：内部命令发出后 1500ms 内忽略 APP 回传的强度值
- 产出 `DeviceCommand`

**新增文件：**
```
src/engine/gift.rs       # impl AppController (gift handling)
src/engine/strength.rs   # impl AppController (strength management + decay)
src/engine/limits.rs     # 安全限制计算
```

**完成条件：**
- 不依赖 B站、不依赖 Coyote，直接通过手动 API 调强度，Panel 能看到变化
- gift 叠加 + 上限截断 + 衰减 floor 计算 + 紧急停止 的组合场景单测通过
- `setManualStrength` 设置 A=80（上限=100），模拟 3 个 gift 各 +10 在有效期内，衰减后强度正确

---

## Phase 5 — Coyote Protocol + Hub

**目标：** 实现 DG-LAB SOCKET v2 协议，支持配对和指令下发。

**内容：**
- 定义 DG-LAB 协议错误码常量
- 实现消息解析（JSON → 结构化 `CoyoteMessage`）和消息构建（结构化 → JSON 字符串）
- 实现 APP 强度反馈消息解析、通道类型安全校验
- 实现 `PairingManager`：普通 struct（`HashMap<ClientId, ClientId>` + 反向映射），不内置锁，并发安全由 `CoyoteHub` 外层保证
- 实现 `CoyoteHub`：管理 WS 连接、消息路由、消息来源验证、配对关系验证、虚拟前端 client
- 接收 `DeviceCommand` 翻译为 APP 协议指令并发送
- APP feedback 转换为 `AppCommand` 推入 Command channel
- 实现 QR 码生成（`ws://host:port/clientId` 格式），输出 SVG base64 data URL（`data:image/svg+xml;base64,...`），避免引入 PNG/image 依赖
- 实现波形定时发送（覆盖保护、按 client 批量清理）
- Coyote heartbeat 由 CoyoteHub 内部管理，不经过 Controller

**新增文件：**
```
src/coyote/
src/coyote/mod.rs
src/coyote/error_codes.rs
src/coyote/protocol.rs    # 消息解析/构建/验证
src/coyote/pairing.rs     # UUID 配对管理
src/coyote/pulse.rs       # 波形定时发送
src/coyote/server.rs      # CoyoteHub + WS server (端口 9999)
```

**完成条件：**
- 消息解析/构建的往返测试通过
- `PairingManager` 关系不变量测试通过（pair/unpair/isPairedWith 状态一致性）
- `CoyoteHub` 并发连接/断连集成测试通过
- timer 启动/停止/覆盖的时序单测通过
- DG-LAB APP 可扫码配对
- 手动强度 API 能够控制 APP 端强度变化

---

## Phase 6 — Bilibili Client + Parser

**目标：** 能连接 B站开放平台，接收弹幕礼物。

**内容：**
- 实现 HMAC-SHA256 + MD5 签名生成函数（纯函数，无状态）
- 封装 B站开放平台 REST 调用：`start` / `end` / `heartbeat`
- 处理 `code=7002`（残留会话自动重试）和 `code=7001`（冷却期拒绝）
- 管理 `game_id` 生命周期：进程启动时清理残留会话，正常结束时置空
- `BilibiliClient` 内部管理 HTTP heartbeat loop，不经过 Controller
- 实现 B站弹幕二进制协议解析（固定 16 字节头 + 变长 body + 多包循环解析）
- 支持 Deflate 和 Brotli 解压后递归解析
- 解析 `LIVE_OPEN_PLATFORM_SEND_GIFT` 消息并转为 `AppCommand::GiftReceived`
- 实现心跳维持和断线重连（指数退避 + 多地址轮换），由 DanmakuClient 内部管理
- 连接状态变化推送 `AppCommand::BilibiliStatusChanged`

**新增文件：**
```
src/bilibili/
src/bilibili/mod.rs
src/bilibili/signer.rs    # HMAC-SHA256 + MD5 签名
src/bilibili/client.rs    # REST API 客户端 + 会话管理
src/bilibili/parser.rs    # 二进制协议解析
src/bilibili/danmaku.rs   # WS 客户端 + 重连 + 心跳
```

**完成条件：**
- 签名正确性单测通过（对照 B站开放平台文档的示例参数）
- 弹幕二进制协议解析单测通过（多包、压缩、边界 case）
- Mock server 测试通过
- 真实 B站联调可收到礼物事件

---

## Phase 7 — 端到端打通

**目标：** 组装所有组件，全链路可运行。

**内容：**
- `main.rs` 中按顺序创建所有 channel 和模块实例，注入所需 sender/receiver
- `tokio::spawn` 启动：Controller、BilibiliClient、CoyoteHub、面板广播转发 task
- `tokio::join!` 并行启动两个 axum server（HTTP + Coyote）和信号处理
- 优雅退出：收到 SIGINT/SIGTERM → 推 `Shutdown` → Controller 顺序清理（停心跳、停衰减、end game、关 WS）→ 各 server 关闭

**修改文件：**
```
src/main.rs              # 扩展到完整实现
src/engine/controller.rs # 完善主循环的 select!
src/engine/bilibili.rs   # impl AppController (bilibili lifecycle)
src/engine/coyote.rs     # impl AppController (coyote pairing/feedback)
```

**完成条件：** 端到端链路：
```
B站礼物 → GiftReceived → StrengthEngine → DeviceCommand → CoyoteHub → APP
                                 ↓
                              AppView → Panel WS
```
手动验证：连接真实 B站 + 真实 DG-LAB APP，发送测试礼物，Panel 显示强度变化。

---

## Phase 8 — 可观测性 + Release

**目标：** 运行时可见、可调试，生产环境可部署。

**内容：**
- 结构化日志：所有关键操作带 span
- `RUST_LOG` 环境变量支持，按模块开关日志级别
- 健康检查端点 `GET /api/health`
- release profile 优化确认
- 单二进制编译验证
- 编写 `justfile`：封装 `build` / `run` / `test` / `release` 命令

**新增文件：**
```
justfile
```

**完成条件：**
- 设置 `RUST_LOG=debug` 启动，看到完整异步 task 调用链追踪
- `GET /api/health` 返回各模块状态
- `cargo build --release` 产物可独立运行，不需要外部 `public/` 目录

---

## Phase 9 — 加固

**目标：** 安全防护和输入校验。

**内容：**
- 面板 API 添加 Token 鉴权（简单方案：配置文件中的共享密钥）
- host 默认改为 `127.0.0.1`（显式配置 `0.0.0.0` 才暴露公网）
- Config 更新接口增加输入校验（端口范围、强度值范围）
- 强度范围校验（0-200）
- rate limiting（可选，低频场景优先级低）

**修改文件：**
```
src/config.rs            # 增加配置校验
src/api/router.rs        # 增加 auth middleware
src/api/config.rs        # 输入校验
```

**完成条件：** 非法输入被正确拒绝；无 token 的请求返回 401

---

## Phase 10 — 真实设备验证

**目标：** 生产环境等价验证。

**内容：**
- 真实直播间长时间运行测试
- 真实 DG-LAB APP + 真实设备联调
- 断线重连验证（关闭 WiFi → 恢复 → 状态正确）
- 异常退出清理验证（`kill -9` 后重启 → game_id 残留可清理 → 不影响下次连接）
- 多礼物并发场景验证
- release binary 在目标环境部署验证

**完成条件：**
- 连续运行 2 小时无崩溃
- 断 WiFi 30s 后恢复，弹幕和强度状态自动恢复
- `kill -9` 重启后能正常 start（残留 game_id 被清理，不触发冷却期）

---

## 依赖关系

```
Phase 1
  └─→ Phase 2
        ├─→ Phase 3 (API + Panel + 静态文件) ──┐
        ├─→ Phase 4 (Strength Engine) ─────────┤
        ├─→ Phase 5 (Coyote Protocol) ─────────┤
        └─→ Phase 6 (Bilibili Client) ─────────┤
              └─→ Phase 7 (端到端打通) ─────────┤
                    ├─→ Phase 8 (可观测性) ────┤
                    ├─→ Phase 9 (加固) ──────┤
                    └─→ Phase 10 (真实验证) ───┘
```

Phase 3、4、5、6 可在 Phase 2 完成后并行开发。Phase 7 需要 3-6 全部完成后才能组装。

---

## 里程碑

| 里程碑 | 完成的 Phase | 可验证的产出 |
|--------|-------------|-------------|
| M1 | 1-2 | `cargo build` + `cargo test` 通过，最小 Controller 可运行 |
| M2 | 3 | 浏览器打开面板，API + Panel WS 通路正常 |
| M3 | 4 | 手动强度 API 可控制，衰减逻辑正确 |
| M4 | 5-6 | APP 可配对 + B站可收礼物（可独立验证） |
| M5 | 7 | **全链路打通：礼物→强度→设备→面板** |
| M6 | 8-10 | 加固完成，真实环境验证通过，可发布 |

---

## 目录结构总览

```
src/
├── main.rs                  # 启动入口，组装所有组件
├── lib.rs                   # 库 root
├── config.rs                # Config struct + ConfigStore
├── command.rs               # AppCommand 枚举
├── view.rs                  # AppView 枚举
├── device.rs                # DeviceCommand 枚举
├── error.rs                 # AppError 枚举 (thiserror)
├── static_files.rs          # rust-embed + 静态文件服务
│
├── bilibili/
│   ├── mod.rs
│   ├── signer.rs            # HMAC-SHA256 + MD5 签名
│   ├── client.rs            # REST API 客户端 + 会话管理
│   ├── parser.rs            # 二进制协议解析
│   └── danmaku.rs           # WS 客户端 + 重连 + 心跳
│
├── coyote/
│   ├── mod.rs
│   ├── server.rs            # CoyoteHub + WS server
│   ├── protocol.rs          # 消息解析/构建/验证
│   ├── pairing.rs           # UUID 配对管理
│   ├── pulse.rs             # 波形定时发送
│   └── error_codes.rs       # 协议错误码
│
├── engine/
│   ├── mod.rs
│   ├── controller.rs        # AppController struct + 主循环
│   ├── gift.rs              # 礼物映射
│   ├── strength.rs          # 强度管理 + 衰减
│   ├── limits.rs            # 安全限制计算
│   ├── bilibili.rs          # B站生命周期处理
│   └── coyote.rs            # Coyote 配对/反馈处理
│
└── api/
    ├── mod.rs
    ├── router.rs            # axum::Router 组装
    ├── bilibili.rs          # /api/bilibili/* handlers
    ├── coyote.rs            # /api/coyote/* handlers
    ├── config.rs            # /api/config/* handlers
    └── panel_ws.rs          # /ws/panel WebSocket handler
```

`public/` 目录原样保留，编译时通过 `rust-embed` 嵌入二进制。

---

## 参考项目

项目中 `.temp/` 目录下保留了三个参考项目，迁移开发时可按需查阅：

| 目录 | 说明 | 关键参考内容 |
|------|------|-------------|
| `.temp/bilive-coyote-ts/` | 本项目的 TS 原版实现 | 完整业务逻辑、前端 UI、API 路由，迁移时作为功能等价性对照 |
| `.temp/bilisdk/` | B站开放平台官方 Demo | `server/tool/index.ts` — 签名算法；`server/routes/` — 开放平台 API 调用方式 |
| `.temp/DG-LAB-OPENSOURCE/` | DG-LAB Coyote 开源仓库 | `socket/v2/README.md` — SOCKET v2 协议完整文档；`socket/v2/backend/src/` — Node.js 参考实现 |

### 关键参考路径

**B站签名算法：**
- 本项目原版：[`.temp/bilive-coyote-ts/src/bilibili/signer.ts`](file:///home/zclkkk/workspace/bilive-coyote/.temp/bilive-coyote-ts/src/bilibili/signer.ts)
- B站官方 Demo：[`.temp/bilisdk/server/tool/index.ts`](file:///home/zclkkk/workspace/bilive-coyote/.temp/bilisdk/server/tool/index.ts)

**弹幕二进制协议：**
- 本项目原版：[`.temp/bilive-coyote-ts/src/bilibili/danmaku-ws.ts`](file:///home/zclkkk/workspace/bilive-coyote/.temp/bilive-coyote-ts/src/bilibili/danmaku-ws.ts)

**DG-LAB SOCKET 协议：**
- 协议文档：[`.temp/DG-LAB-OPENSOURCE/socket/v2/README.md`](file:///home/zclkkk/workspace/bilive-coyote/.temp/DG-LAB-OPENSOURCE/socket/v2/README.md)
- Node.js 服务端：[`.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/index.js`](file:///home/zclkkk/workspace/bilive-coyote/.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/index.js)
- 连接管理：[`.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/connection.js`](file:///home/zclkkk/workspace/bilive-coyote/.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/connection.js)
- 消息路由：[`.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/message.js`](file:///home/zclkkk/workspace/bilive-coyote/.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/message.js)
- 定时器：[`.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/timer.js`](file:///home/zclkkk/workspace/bilive-coyote/.temp/DG-LAB-OPENSOURCE/socket/v2/backend/src/timer.js)

**业务逻辑对照：**
- 礼物映射：[`.temp/bilive-coyote-ts/src/engine/gift-mapper.ts`](file:///home/zclkkk/workspace/bilive-coyote/.temp/bilive-coyote-ts/src/engine/gift-mapper.ts)
- 强度管理：[`.temp/bilive-coyote-ts/src/engine/strength-manager.ts`](file:///home/zclkkk/workspace/bilive-coyote/.temp/bilive-coyote-ts/src/engine/strength-manager.ts)
- 前端面板：[`.temp/bilive-coyote-ts/public/`](file:///home/zclkkk/workspace/bilive-coyote/.temp/bilive-coyote-ts/public/)

---

## Cargo.toml 目标依赖

Phase 1 启动时应锁定最新版本，下表为大版本参考：

| Crate | 用途 | 大版本 |
|-------|------|--------|
| `tokio` | 异步运行时 | `1` (features: `full`) |
| `axum` | HTTP + WebSocket 框架 | `0.8` (features: `ws`) |
| `tower` | axum 中间件基础设施 | `0.5` |
| `tower-http` | 常用中间件 (cors, fs) | `0.6` (features: `fs`, `cors`) |
| `tokio-tungstenite` | 弹幕 WS 客户端 | `0.29` |
| `serde` | 序列化框架 | `1` (features: `derive`) |
| `serde_json` | JSON 序列化 | `1` |
| `bytes` | 零拷贝字节缓冲 | `1` |
| `hmac` | HMAC 签名 | `0.12` |
| `sha2` | SHA-256 哈希 | `0.10` |
| `md-5` | MD5 哈希 | `0.10` |
| `hex` | Hex 编解码 | `0.4` |
| `flate2` | Deflate 解压 | `1` |
| `brotli` | Brotli 解压 | `7` |
| `tracing` | 结构化日志 | `0.1` |
| `tracing-subscriber` | 日志订阅器 | `0.3` (features: `env-filter`) |
| `thiserror` | 错误类型派生 | `2` |
| `anyhow` | 应用级错误处理 | `1` |
| `uuid` | UUID v4 生成 | `1` (features: `v4`) |
| `chrono` | 时间戳处理 | `0.4` |
| `rust-embed` | 静态文件嵌入 | `8` |
| `qrcode` | QR 码 SVG 生成 | `0.14` |

> **注意：** 上表为大版本参考，具体 minor/patch 版本在 Phase 1 初始化时通过 `cargo add` 或查看 `crates.io` 锁定最新。
