# bilive-coyote

Bilibili 直播礼物 → DG-LAB Coyote 强度 LAN 桥接服务。

## 功能

- 接收 B站直播礼物（开放平台 / 弹幕广播两种来源）
- 按规则映射为 DG-LAB Coyote 设备强度指令
- 通过局域网 WebSocket 控制 Coyote App
- Web 控制面板实时查看状态

## 构建

```bash
cargo build --release
```

## 运行

```bash
# 使用默认配置文件
./target/release/bilive-coyote

# 指定配置和状态文件
./target/release/bilive-coyote --config my-config.json --state my-state.json

# 环境变量方式
CONFIG_PATH=my-config.json STATE_PATH=my-state.json ./target/release/bilive-coyote
```

首次运行会在当前目录生成 `config.json`，编辑后填入 B站开放平台凭证或直播间房间号。

## 配置

编辑 `config.json`：

```json
{
  "bilibili": {
    "source": "broadcast",
    "openPlatform": { "appKey": "", "appSecret": "", "code": "", "appId": 0 },
    "broadcast": { "roomId": 0 }
  },
  "coyote": { "wsPort": 9999 },
  "server": { "httpPort": 3000, "host": "0.0.0.0" },
  "rules": [
    { "giftName": "小心心", "coinType": "silver", "channel": "A", "strengthAdd": 5, "duration": 10 }
  ],
  "safety": { "limitA": 80, "limitB": 80, "decayEnabled": true, "decayRate": 2 }
}
```

## API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/status` | 全局状态 |
| POST | `/api/bilibili/start` | 启动 B站连接 |
| POST | `/api/bilibili/stop` | 停止 B站连接 |
| GET | `/api/coyote/status` | Coyote 配对状态 |
| GET | `/api/coyote/qrcode` | 获取配对二维码 |
| POST | `/api/coyote/strength` | 手动设置强度 |
| POST | `/api/coyote/emergency` | 紧急停止 |
| GET/PUT | `/api/config` | 读写配置 |
| GET/PUT | `/api/config/rules` | 读写规则 |
| WS | `/ws/panel` | 面板事件推送 |

## 技术栈

Rust · Tokio · Axum · tungstenite · reqwest · serde

## 许可证

本项目采用 `Parity Public License 7.0.0` 授权，详见根目录 `LICENSE`。

如果你使用本软件开发、运行或分析其他软件，则相关软件也需要按照协议要求开放共享。
