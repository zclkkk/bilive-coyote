# bilive-coyote

Bilibili 直播礼物到 DG-LAB Coyote 3.0 的局域网 WebSocket 桥接服务。

项目内置控制面板、B 站礼物监听、礼物规则引擎、DG-LAB APP Socket 配对服务和官方 V3 波形输出。启动后用 DG-LAB APP 扫描面板二维码，礼物或手动操作会转换为 Coyote A/B 通道强度和波形。

## 功能

- B 站直播礼物监听：开放平台 / Broadcast 弹幕广播
- DG-LAB APP Socket 配对：二维码、状态反馈、强度反馈
- 礼物规则：强度加成、持续时间、波形切换
- 强度生命周期：安全上限、APP 上限、衰减、紧急停止
- 波形输出：内置官方 V3 波形，强度大于 0 时自动持续补发
- Web 控制面板：状态、配对、强度、波形、规则、安全配置、日志

## 快速开始

```bash
cargo build --release
./target/release/bilive-coyote
```

打开控制面板：

```txt
http://localhost:3000
```

用 DG-LAB APP 的 Socket 功能扫描面板二维码完成配对。

## 文档

- [安装与运行](docs/setup.md)
- [配置与礼物规则](docs/configuration.md)
- [B 站数据源](docs/bilibili-sources.md)
- [DG-LAB Coyote 与波形](docs/coyote.md)
- [HTTP 与 WebSocket API](docs/api.md)
- [开发与架构](docs/development.md)

## 技术栈

Rust · Tokio · Axum · tungstenite · reqwest · serde

## 许可证

本项目采用 `Parity Public License 7.0.0` 授权，详见根目录 `LICENSE`。

如果你使用本软件开发、运行或分析其他软件，则相关软件也需要按照协议要求开放共享。
