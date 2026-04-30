interface PulseTask {
  timer: ReturnType<typeof setInterval>
  clientId: string
  targetId: string
  channelNum: number
}

export class PulseTimerManager {
  private tasks: Map<string, PulseTask> = new Map()
  private sendFn: (clientId: string, targetId: string, message: string) => void
  private notifyFn: (clientId: string, targetId: string, message: string) => void

  constructor(
    sendFn: (clientId: string, targetId: string, message: string) => void,
    notifyFn: (clientId: string, targetId: string, message: string) => void,
  ) {
    this.sendFn = sendFn
    this.notifyFn = notifyFn
  }

  startPulse(
    key: string,
    clientId: string,
    targetId: string,
    channel: string,
    hexArray: string[],
    duration: number,
    frequency: number = 1,
  ): void {
    this.stopPulse(key)

    const channelNum = channel === "A" ? 1 : 2

    const pulseMsg = `pulse-${channel}:${JSON.stringify(hexArray)}`
    const totalSends = frequency * duration
    let sent = 0

    this.sendFn(clientId, targetId, pulseMsg)
    sent++

    const timer = setInterval(() => {
      if (sent >= totalSends) {
        this.stopPulse(key)
        this.notifyFn(clientId, targetId, "发送完毕")
        return
      }
      this.sendFn(clientId, targetId, pulseMsg)
      sent++
    }, 1000 / frequency)

    this.tasks.set(key, { timer, clientId, targetId, channelNum })
  }

  stopPulse(key: string): void {
    const task = this.tasks.get(key)
    if (task) {
      clearInterval(task.timer)
      this.sendFn(task.clientId, task.targetId, `clear-${task.channelNum}`)
      this.tasks.delete(key)
    }
  }

  stopPulseByClient(clientId: string): void {
    for (const [key, task] of this.tasks) {
      if (task.clientId === clientId) {
        clearInterval(task.timer)
        this.sendFn(task.clientId, task.targetId, `clear-${task.channelNum}`)
        this.tasks.delete(key)
      }
    }
  }

  stopAll(): void {
    for (const [, task] of this.tasks) {
      clearInterval(task.timer)
      this.sendFn(task.clientId, task.targetId, `clear-${task.channelNum}`)
    }
    this.tasks.clear()
  }
}
