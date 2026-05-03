# DG-LAB Coyote

项目内置 DG-LAB Coyote WebSocket 服务端，默认监听 `0.0.0.0:9999`。

控制面板会展示配对二维码。用 DG-LAB APP 扫码后，APP 会连接到这个 WebSocket 服务。

## 局域网访问

默认配置允许局域网访问：

```json
{
  "server": {
    "httpPort": 3000,
    "host": "0.0.0.0"
  },
  "coyote": {
    "wsPort": 9999
  }
}
```

手机访问控制面板时使用运行机器的局域网 IP，例如：

```txt
http://192.168.1.20:3000
```

## 配对

1. 启动项目。
2. 打开控制面板。
3. 用 DG-LAB APP 扫描配对二维码。
4. 等待控制面板显示 Coyote 已连接。

配对后，礼物规则、手动调整、衰减和紧急停止都可以改变 A/B 通道强度。内部实现见 [开发与架构](development.md#礼物到强度)。

## 上限

最终强度上限取两者较小值：

```txt
min(本端 safety.limitA/B, APP 端上限)
```

APP 端上限来自 DG-LAB APP 的状态反馈。本端 safety 上限来自配置：

```json
{
  "safety": {
    "limitA": 80,
    "limitB": 80,
    "decayEnabled": true,
    "decayRate": 2
  }
}
```

## 衰减

礼物规则有持续时间。持续时间结束后，项目会按 `decayRate` 逐步把强度拉回基线。

关闭衰减：

```json
{
  "safety": {
    "decayEnabled": false
  }
}
```

## 紧急停止

控制面板的「紧急停止」会把 A/B 通道归零。Coyote WS 断连时也会自动归零本地强度。
