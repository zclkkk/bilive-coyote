# DG-LAB Coyote 与波形

项目内置 DG-LAB APP Socket 服务，默认监听：

```txt
0.0.0.0:9999
```

控制面板生成二维码。DG-LAB APP 扫码后会连接到这个 WebSocket 服务，项目再通过 APP 转发指令给 Coyote 3.0。

## 配对

配对二维码内容格式：

```txt
https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#ws://<host>:<port>/<bridgeId>
```

配对流程：

1. APP 连接 WebSocket。
2. 服务端分配 APP 端临时 ID。
3. APP 发送 `bind`。
4. 服务端验证二维码中的 bridge id。
5. 配对成功后开始接受强度反馈和发送控制指令。

同一时刻只维护一个已配对 APP。新的 APP 配对成功会断开旧配对。

## 强度指令

项目发给 APP 的强度指令：

```txt
strength-通道+模式+数值
```

通道：

- `1` = A
- `2` = B

模式：

- `0` = 减少
- `1` = 增加
- `2` = 设置到指定值

项目内部主要使用绝对设置：

```txt
strength-1+2+30
strength-2+2+20
```

APP 会反馈当前强度和软上限：

```txt
strength-A强度+B强度+A上限+B上限
```

例如：

```txt
strength-11+7+100+35
```

## 波形指令

DG-LAB APP Socket 中，强度和波形是两类指令：

```txt
strength-通道+模式+数值
pulse-通道:["HEX波形数据", ...]
```

只设置强度不会产生输出。项目保持以下规则：

```txt
通道强度 > 0  => 持续补发当前波形
通道强度 = 0  => 停止补发并 clear 通道队列
```

波形指令示例：

```txt
pulse-A:["0A0A0A0A00000000","0A0A0A0A14141414"]
pulse-B:["0A0A0A0A00000000","0A0A0A0A14141414"]
```

每个 frame 是 8 字节 HEX，代表 100ms 波形数据。项目内置官方 V3 `expectedV3` 数据。

## 波形选择

每个通道维护一个当前波形：

- A 默认 `breath`
- B 默认 `breath`

面板选择波形时：

- 如果通道强度为 0，只更新当前选择。
- 如果通道强度大于 0，立即切换正在输出的波形。

礼物规则也可以切换波形：

```json
{
  "waveform": "tide"
}
```

或切到下一个：

```json
{
  "waveform": "next"
}
```

## 覆盖和停止

同一通道切换波形时：

1. 停止旧的本地补发任务。
2. 发送 `clear-1` 或 `clear-2` 清空 APP 队列。
3. 等待约 150ms。
4. 启动新波形补发任务。

强度归零、紧急停止、APP 断连时会停止对应波形并清空队列。

## 紧急停止

紧急停止会：

1. A/B 强度设置为 0。
2. 停止 A/B 本地波形任务。
3. 发送 `clear-1` 和 `clear-2`。
4. 清空本地强度状态和礼物加成到期记录。
