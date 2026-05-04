# 开发与架构

## 目录

```txt
src/
  app.rs                  应用组装和任务启动
  main.rs                 入口
  cli.rs                  CLI 参数
  bilibili/               B 站开放平台、Broadcast、直播 WS 协议
  config/                 配置结构、校验、持久化
  coyote/                 DG-LAB APP Socket 协议、配对、强度和波形下发
  engine/                 礼物规则、强度生命周期、衰减
  http/                   REST API、面板 WS、静态文件
web/
  index.html              控制面板
  css/main.css
  js/
    api.js
    panel.js
    ws.js
    utils.js
_archive/                 TypeScript 原项目归档
DG-LAB-OPENSOURCE/        官方开源资料，本地参考目录，已加入 git exclude
```

## 任务模型

启动时创建：

- `BilibiliManager`
- `CoyoteManager`
- `StrengthEngine`
- HTTP server
- Coyote APP Socket server
- 面板事件广播任务

核心通道：

```txt
BilibiliManager --GiftEvent--> StrengthEngine --CoyoteCommand--> CoyoteManager
                                           |
                                           +--PanelEvent--> Web panel
```

## B 站接入

`src/bilibili` 下有两套数据源：

- `open_platform`
- `broadcast`

它们都输出 `GiftEvent`。后续规则匹配不关心来源。

## 配置

配置结构定义在 `src/config/types.rs`。

校验逻辑在 `src/config/validation.rs`：

- 端口、host
- safety 上限
- 礼物规则
- 波形 ID
- 手动强度输入
- B 站启动参数

配置文件不存在时使用默认配置。调用配置更新 API 时会持久化到指定路径。

## 礼物到强度

`StrengthEngine` 负责：

- 当前强度
- baseline
- 礼物加成到期记录
- APP 上限
- 本端 safety 上限
- 衰减
- 手动强度
- 紧急停止
- 规则更新

礼物处理流程：

1. 按规则顺序找第一条匹配规则。
2. 如果规则有 `waveform`，先发送波形选择命令。
3. 如果 `strengthAdd > 0`，按数量计算强度增量。
4. 对目标通道应用有效上限。
5. 发送强度指令。
6. 强度大于 0 时发送 `EnsureWaveform`。
7. 发出面板日志。

## 衰减

每秒执行一次。

流程：

1. 移除已经到期的加成。
2. 计算仍然有效的加成 floor。
3. 如果当前强度高于 floor，则按 `decayRate` 下降。
4. 下降后同步强度和波形。

## Coyote Manager

`CoyoteManager` 负责：

- APP 配对状态
- 强度指令下发
- clear 指令下发
- A/B 当前波形选择
- A/B 波形补发任务
- 心跳

波形命令：

- `SelectWaveform`
- `NextWaveform`
- `EnsureWaveform`
- `StopWaveform`

同通道切换波形时，先停止旧任务并清空 APP 队列，再延迟约 150ms 启动新任务。

## 波形 catalog

`src/coyote/waveform.rs` 内置官方 V3 `expectedV3` 数据。

校验要求：

- 每个 frame 是 16 位 HEX
- 每个 preset frame 数不超过 100
- `pulse-*` 消息不超过协议限制

## HTTP 和前端

HTTP 路由在 `src/http/mod.rs` 和 `src/http/routes.rs`。

静态文件来自 `web/`，由 `rust-embed` 内嵌。

面板事件通过 `/ws/panel` 广播。前端自动重连后会重新拉取配置、状态和波形。

## 检查

```bash
cargo fmt --check
cargo test
node --check web/js/panel.js
node --check web/js/api.js
```

严格 clippy 当前会命中一个既有的 `src/bilibili/live_socket/mod.rs` 样式 lint，和波形功能无关。
