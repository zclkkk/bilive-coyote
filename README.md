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

## 开放平台配置

开放平台适合正式接入，需要在控制面板填写 `App ID`、`AppKey`、`AppSecret` 和 `主播身份码`。

1. 打开 [B 站直播开放平台](https://open-live.bilibili.com/open-manage)。
2. 点进自己的项目；如果没有项目，先创建一个。
3. 在项目页找到 `项目ID`，填到控制面板的 `App ID`。
4. 在开放平台的个人资料里找到 `access_key_id` 和 `access_key_secred`，分别填到 `AppKey` 和 `AppSecret`。
5. 打开 [B 站直播中心](https://link.bilibili.com/p/center/index)，进入 `我的直播间` -> `开播设置`。
6. 复制 `身份码`，填到控制面板的 `主播身份码`。

## Broadcast 登录 JSON

Broadcast 可以只填房间号启动；如果游客态收不到礼物 `cmd`，在控制面板粘贴 BiliTV 登录 JSON。这份 JSON 可以用 biliupR 生成。

1. 打开 [biliup Releases](https://github.com/biliup/biliup/releases/latest)。
2. 下载与你系统匹配的 biliupR 压缩包并解压。
3. 在解压目录运行登录命令：

```bash
./biliup login
```

Windows PowerShell：

```powershell
.\biliup.exe login
```

4. 按终端提示完成登录。biliupR 默认会在当前目录生成 `cookies.json`。
5. 打开 `cookies.json`，复制完整 JSON 到控制面板的 `BiliTV 登录 JSON`，填写房间号后开始监听。

`cookies.json` 等同登录凭据，不要提交或分享；控制面板只把它用于本次 Broadcast 启动，不写入配置文件。

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
