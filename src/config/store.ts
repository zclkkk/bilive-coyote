import { AppConfig, DEFAULT_CONFIG } from "./types"
import { existsSync, readFileSync } from "fs"
import { resolve, isAbsolute } from "path"

export class ConfigStore {
  private data: AppConfig
  private filePath: string

  constructor() {
    // 优先使用环境变量 CONFIG_PATH，其次以 cwd 为基准 (兼容 bun build --compile 后的二进制)
    const envPath = process.env.CONFIG_PATH
    if (envPath && envPath.length > 0) {
      this.filePath = isAbsolute(envPath) ? envPath : resolve(process.cwd(), envPath)
    } else {
      this.filePath = resolve(process.cwd(), "config.json")
    }
    this.data = { ...DEFAULT_CONFIG }
    if (existsSync(this.filePath)) {
      try {
        const content = readFileSync(this.filePath, "utf-8")
        const parsed = JSON.parse(content)
        this.data = deepMerge(this.data, parsed)
      } catch (e) {
        console.error(`[Config] Failed to parse ${this.filePath}:`, e)
      }
    }
  }

  get(): AppConfig {
    return this.data
  }

  get bilibili() {
    return this.data.bilibili
  }

  get coyote() {
    return this.data.coyote
  }

  get server() {
    return this.data.server
  }

  get rules() {
    return this.data.rules
  }

  get safety() {
    return this.data.safety
  }

  async set(partial: Partial<AppConfig>): Promise<void> {
    this.data = deepMerge(this.data, partial)
    await this.save()
  }

  async setRules(rules: AppConfig["rules"]): Promise<void> {
    this.data.rules = rules
    await this.save()
  }

  async save(): Promise<void> {
    try {
      await Bun.write(this.filePath, JSON.stringify(this.data, null, 2))
    } catch (e) {
      console.error("[Config] Failed to save:", e)
    }
  }
}

function deepMerge<T extends Record<string, any>>(target: T, source: Partial<T>): T {
  const result = { ...target }
  for (const key of Object.keys(source) as (keyof T)[]) {
    const sv = source[key]
    const tv = target[key]
    if (sv && typeof sv === "object" && !Array.isArray(sv) && tv && typeof tv === "object" && !Array.isArray(tv)) {
      result[key] = deepMerge(tv as Record<string, any>, sv as Record<string, any>) as T[keyof T]
    } else if (sv !== undefined) {
      result[key] = sv as T[keyof T]
    }
  }
  return result
}
