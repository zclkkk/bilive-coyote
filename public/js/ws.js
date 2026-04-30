export class PanelWS {
  constructor() {
    this.handlers = {}
    this.ws = null
    this.reconnectTimer = null
    this.hasConnectedBefore = false
    this.connect()
  }

  connect() {
    const proto = location.protocol === "https:" ? "wss:" : "ws:"
    this.ws = new WebSocket(`${proto}//${location.host}/ws/panel`)

    this.ws.onopen = () => {
      console.log("[PanelWS] Connected")
      if (this.reconnectTimer) {
        clearTimeout(this.reconnectTimer)
        this.reconnectTimer = null
      }
      // 重连成功时触发 reconnect 事件，让上层重新拉取状态
      if (this.hasConnectedBefore) {
        this.emit("reconnect", null)
      }
      this.hasConnectedBefore = true
    }

    this.ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data)
        this.emit(msg.type, msg.data)
      } catch {}
    }

    this.ws.onclose = () => {
      console.log("[PanelWS] Disconnected, reconnecting...")
      this.reconnectTimer = setTimeout(() => this.connect(), 3000)
    }

    this.ws.onerror = () => {}
  }

  on(event, handler) {
    if (!this.handlers[event]) this.handlers[event] = []
    this.handlers[event].push(handler)
  }

  emit(event, data) {
    if (!this.handlers[event]) return
    for (const h of this.handlers[event]) h(data)
  }
}
