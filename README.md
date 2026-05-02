# Bilive-Coyote

Bilibili 直播礼物 → DG-LAB Coyote 强度桥接。

收到直播间礼物后自动增加 Coyote 电击强度。

## 特性

- 🎁 B站直播间礼物实时触发 Coyote 强度变化
- 📱 Web 控制面板，手机/PC 均可访问
- 🔒 强度上限/衰减/紧急停止
- 🌍 跨平台：Windows / Linux / macOS（Bun 运行时）

## 快速开始

```bash
# 安装 Bun (如果没装)
curl -fsSL https://bun.sh/install | bash

# 安装依赖
bun install

# 启动
bun run src/main.ts
```

打开 `http://localhost:3000` 进入控制面板。

## 使用流程

1. 在控制面板填写 B站开放平台的 **AppKey**、**AppSecret**、**主播身份码**、**App ID**
2. 点击「开始监听」连接直播间
3. 在 Coyote 配对区域扫描二维码，用 DG-LAB APP 完成配对
4. 配置礼物规则（礼物名 → 通道 → 强度增量 → 持续时间）
5. 观众送礼 → 强度自动变化

## 技术栈

| 层 | 技术 |
|---|---|
| 运行时 | Bun |
| 语言 | TypeScript |
| HTTP/WS | Bun.serve() 内置 |
| 弹幕协议 | 自实现二进制协议解析 + Deflate/Brotli 解压 |
| 前端 | 原生 HTML + CSS + JS (ES Module, 零构建) |
| 外部依赖 | qrcode (唯一) |

## 项目结构

```
src/
  main.ts                # 入口
  bilibili/
    signer.ts            # HMAC-SHA256 + MD5 签名
    api.ts               # B站开放平台 API
    danmaku-ws.ts        # 弹幕 WebSocket 客户端
  coyote/
    server.ts            # DG-LAB WS 服务端 (端口 9999)
    message.ts           # 消息解析与构造
    error-codes.ts       # 协议错误码
  engine/
    event-bus.ts         # 类型安全事件总线
    gift-mapper.ts       # 礼物→强度映射
    strength-manager.ts  # 强度管理 + 衰减
  server/
    main-server.ts       # 主服务 (端口 3000)
    router.ts            # REST API 路由
  config/
    types.ts             # 配置类型定义
    store.ts             # 配置持久化
public/
  index.html             # 控制面板
  css/                   # 样式
  js/                    # 前端逻辑
```

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | /api/bilibili/start | 开始监听直播间 |
| POST | /api/bilibili/stop | 停止监听 |
| GET | /api/bilibili/status | B站连接状态 |
| GET | /api/coyote/status | Coyote 配对状态 |
| GET | /api/coyote/qrcode | 配对二维码 (Base64) |
| POST | /api/coyote/strength | 手动设置强度 |
| POST | /api/coyote/emergency | 紧急停止 |
| GET | /api/config | 获取配置 |
| PUT | /api/config | 更新配置 |
| GET | /api/config/rules | 获取礼物规则 |
| PUT | /api/config/rules | 更新礼物规则 |

## 安全机制

- **强度上限**：A/B 通道强度不可超越的上限 (0-200)，取 min(本端设置, APP端上限)
- **衰减**：礼物效果到期后逐步衰减回基线
- **紧急停止**：一键归零所有通道 + 清空波形队列
- **断连保护**：Coyote WS 断连时自动归零

## 数据流

```
直播间礼物 → B站弹幕WS(op=5) → 解析SEND_GIFT → EventBus
  → GiftMapper匹配规则 → StrengthManager(安全限制+衰减)
    → CoyoteServer → DG-LAB APP → 蓝牙 → 设备
    → PanelWS → 控制面板
```

## 开发

```bash
# 开发模式 (热重载)
bun run --watch src/main.ts

# 编译为单二进制
bun build src/main.ts --compile --outfile bilive-coyote
```
