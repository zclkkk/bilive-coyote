# 安装与分发

## 安装

项目使用 Bun 运行和构建。

```bash
curl -fsSL https://bun.sh/install | bash
bun install
```

## 启动

```bash
bun run start
```

默认会监听两个服务：

| 服务 | 默认监听 | 说明 |
|---|---|---|
| 控制面板 / REST API | `http://0.0.0.0:3000` | 局域网设备可访问 |
| Coyote WS | `ws://0.0.0.0:9999` | DG-LAB APP 扫码后连接 |

浏览器打开 `http://localhost:3000` 即可进入控制面板。手机访问时使用运行机器的局域网 IP。

## 配置文件

默认配置文件是当前工作目录下的 `config.json`。可以用 `CONFIG_PATH` 指定其他位置：

```bash
CONFIG_PATH=/path/to/config.json bun run start
```

单文件可执行程序也使用同样规则：

```bash
CONFIG_PATH=/path/to/config.json ./dist/bilive-coyote
```

## 构建

构建当前平台的单文件可执行程序：

```bash
bun run build
```

构建常见平台：

```bash
bun run build:all
```

单平台命令：

```bash
bun run build:linux-x64
bun run build:linux-arm64
bun run build:windows-x64
bun run build:windows-arm64
bun run build:darwin-x64
bun run build:darwin-arm64
```

产物输出在 `dist/`。前端资源通过 Bun HTML import 内嵌到可执行程序，不需要携带 `public/` 目录。

## 开发命令

```bash
bun run dev      # watch 模式启动
bun run check    # TypeScript 检查
```
