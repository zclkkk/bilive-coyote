# 安装与运行

## 环境

- Rust stable
- 可访问 B 站接口的网络
- DG-LAB APP
- Coyote 3.0 设备

DG-LAB APP 和运行 `bilive-coyote` 的机器需要在同一局域网内，除非你自行暴露 WebSocket 服务并处理 HTTPS/WSS。

## 构建

```bash
cargo build --release
```

开发期也可以直接运行：

```bash
cargo run
```

## 启动

默认启动：

```bash
./target/release/bilive-coyote
```

指定配置和运行状态文件：

```bash
./target/release/bilive-coyote --config config.json --state state.json
```

环境变量方式：

```bash
CONFIG_PATH=config.json STATE_PATH=state.json ./target/release/bilive-coyote
```

默认控制面板地址：

```txt
http://localhost:3000
```

默认 DG-LAB APP Socket 服务：

```txt
ws://<运行机器局域网 IP>:9999/<bridgeId>
```

配置文件不存在时会使用默认配置；通过面板保存配置或调用配置 API 后会写入配置文件。

## 配对流程

1. 启动服务。
2. 打开控制面板。
3. 打开 DG-LAB APP 的 Socket 功能。
4. 用 APP 扫描控制面板二维码。
5. 面板显示 Coyote 已配对后即可控制 A/B 通道。

二维码内容会带上 DG-LAB APP 识别所需前缀：

```txt
https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#
```

如果 `server.host` 是 `0.0.0.0`，二维码会使用运行机器的局域网 IP。

## 常见端口

| 配置 | 默认值 | 说明 |
| --- | --- | --- |
| `server.httpPort` | `3000` | 控制面板和 HTTP API |
| `coyote.wsPort` | `9999` | DG-LAB APP Socket 服务 |

## 检查

```bash
cargo fmt --check
cargo test
```

前端是静态文件，不需要打包。
