# HTTP 与 WebSocket API

控制面板和外部调用共用同一组 HTTP API。

## 状态

### `GET /api/status`

返回全局状态：

- B 站连接状态
- Coyote 配对状态
- 当前强度状态

### `GET /api/bilibili/status`

返回 B 站连接状态。

### `GET /api/coyote/status`

返回 Coyote 配对、当前强度、APP 上限和有效上限。

## B 站

### `POST /api/bilibili/start`

启动监听。

Broadcast：

```json
{
  "source": "broadcast",
  "roomId": 123456
}
```

开放平台：

```json
{
  "source": "open-platform",
  "appKey": "...",
  "appSecret": "...",
  "code": "...",
  "appId": 123
}
```

### `POST /api/bilibili/stop`

停止监听。

## Coyote

### `GET /api/coyote/qrcode`

返回配对二维码 data URL：

```json
{
  "qrcode": "data:image/png;base64,..."
}
```

### `POST /api/coyote/strength`

手动设置强度：

```json
{
  "channel": "A",
  "value": 30
}
```

`channel` 可选：

- `A`
- `B`

`value` 范围：

```txt
0..200
```

实际下发值还会被有效上限裁剪。

### `POST /api/coyote/emergency`

紧急停止：

- A/B 强度归零
- 停止 A/B 波形
- 清空 APP 波形队列

### `GET /api/coyote/waveforms`

返回波形列表和当前选择：

```json
{
  "items": [
    {
      "id": "breath",
      "name": "呼吸",
      "frameCount": 12,
      "durationMs": 1200
    }
  ],
  "selectedA": "breath",
  "selectedB": "breath"
}
```

### `POST /api/coyote/waveform`

选择指定波形：

```json
{
  "action": "select",
  "channel": "A",
  "waveformId": "tide"
}
```

切到下一个波形：

```json
{
  "action": "next",
  "channel": "both"
}
```

`channel` 可选：

- `A`
- `B`
- `both`

## 配置

### `GET /api/config`

获取完整配置。

### `PUT /api/config`

合并更新配置。示例：

```json
{
  "safety": {
    "limitA": 60,
    "limitB": 60
  }
}
```

### `GET /api/config/rules`

获取礼物规则。

### `PUT /api/config/rules`

替换礼物规则。

请求体是规则数组：

```json
[
  {
    "giftName": "小心心",
    "coinType": "silver",
    "channel": "A",
    "strengthAdd": 5,
    "duration": 10
  }
]
```

## 面板 WebSocket

路径：

```txt
/ws/panel
```

事件格式：

```json
{
  "type": "event-name",
  "data": {}
}
```

事件：

| 事件 | 说明 |
| --- | --- |
| `bilibili:status` | B 站连接状态 |
| `coyote:status` | Coyote 配对和强度状态 |
| `strength` | 本地强度变化 |
| `waveform:status` | 当前波形选择变化 |
| `gift` | 礼物日志 |
