import { existsSync, readFileSync } from "fs"
import { resolve, isAbsolute } from "path"

/**
 * 运行时瞬态，和用户配置 (config.json) 分离存储。
 * 目前只存放需要跨进程恢复的会话句柄。
 */
export interface RuntimeState {
  openPlatformGameId: string
}

const DEFAULT_STATE: RuntimeState = {
  openPlatformGameId: "",
}

export class RuntimeStateStore {
  private data: RuntimeState
  private filePath: string

  constructor() {
    const envPath = process.env.STATE_PATH
    if (envPath && envPath.length > 0) {
      this.filePath = isAbsolute(envPath) ? envPath : resolve(process.cwd(), envPath)
    } else {
      this.filePath = resolve(process.cwd(), "state.json")
    }
    this.data = { ...DEFAULT_STATE }
    if (existsSync(this.filePath)) {
      try {
        const parsed = JSON.parse(readFileSync(this.filePath, "utf-8")) as Partial<RuntimeState>
        if (typeof parsed.openPlatformGameId === "string") {
          this.data.openPlatformGameId = parsed.openPlatformGameId
        }
      } catch (e) {
        console.error(`[RuntimeState] Failed to parse ${this.filePath}:`, e)
      }
    }
  }

  get openPlatformGameId(): string {
    return this.data.openPlatformGameId
  }

  async setOpenPlatformGameId(value: string): Promise<void> {
    this.data.openPlatformGameId = value
    await Bun.write(this.filePath, JSON.stringify(this.data, null, 2))
  }
}
