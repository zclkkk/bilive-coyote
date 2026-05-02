# Bilive-Coyote

Bilibili 直播礼物到 DG-LAB Coyote 强度的局域网桥接工具。

收到直播间礼物后，项目会按配置好的礼物规则调整 Coyote A/B 通道强度，并把状态同步到 Web 控制面板。

## 特性

- Bilibili 礼物实时监听
- 支持开放平台和 Broadcast 观众端 WS 两种数据源
- DG-LAB APP 扫码配对，局域网手机/PC 控制面板
- 强度上限、APP 上限反馈、衰减、紧急停止
- Bun 单文件可执行程序分发

## 快速开始

```bash
curl -fsSL https://bun.sh/install | bash
bun install
bun run start
```

打开 `http://localhost:3000` 进入控制面板。

## 使用流程

1. 选择 Bilibili 数据源。
2. 开放平台填写 `AppKey`、`AppSecret`、主播身份码、`App ID`；Broadcast 填写直播间房间号。
3. 点击「开始监听」。
4. 用 DG-LAB APP 扫描 Coyote 配对二维码。
5. 配置礼物规则。

## 常用命令

```bash
bun run dev
bun run check
bun run build
bun run build:all
```

## 文档

- [安装与分发](docs/setup.md)
- [Bilibili 数据源](docs/bilibili-sources.md)
- [DG-LAB Coyote](docs/coyote.md)
- [开发与架构](docs/development.md)

## 技术栈

| 层 | 技术 |
|---|---|
| 运行时 | Bun |
| 语言 | TypeScript |
| HTTP/WS | Bun.serve() |
| Bilibili | 开放平台 / Broadcast 观众端 WS |
| Coyote | DG-LAB App WebSocket 协议 |
| 前端 | 原生 HTML + CSS + JS，Bun HTML import 打包 |
