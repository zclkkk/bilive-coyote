import { randomUUID } from "crypto"

export class PairingManager {
  private pairings: Map<string, string> = new Map()
  private reversePairings: Map<string, string> = new Map()

  pair(clientId: string, targetId: string): boolean {
    if (this.isPaired(clientId) || this.isPaired(targetId)) return false
    this.pairings.set(clientId, targetId)
    this.reversePairings.set(targetId, clientId)
    return true
  }

  unpair(clientId: string): string | null {
    const target = this.pairings.get(clientId)
    if (target) {
      this.pairings.delete(clientId)
      this.reversePairings.delete(target)
      return target
    }
    const source = this.reversePairings.get(clientId)
    if (source) {
      this.reversePairings.delete(clientId)
      this.pairings.delete(source)
      return source
    }
    return null
  }

  isPaired(clientId: string): boolean {
    return this.pairings.has(clientId) || this.reversePairings.has(clientId)
  }

  isPairedWith(a: string, b: string): boolean {
    return this.pairings.get(a) === b || this.reversePairings.get(a) === b
  }

  getPartnerId(clientId: string): string | null {
    return this.pairings.get(clientId) ?? this.reversePairings.get(clientId) ?? null
  }

  generateId(): string {
    return randomUUID()
  }
}
