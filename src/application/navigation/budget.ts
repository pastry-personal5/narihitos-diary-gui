/** Accounted retained UTF-8 payloads, shared by imports, editor documents/history and retry bodies.
 * JS/DOM object overhead is deliberately not represented as a process RSS guarantee. */
export class SessionBudget {
  private readonly allocations = new Map<string, number>();
  constructor(readonly limit = 64 * 1024 * 1024) {}
  get used(): number {
    let total = 0;
    for (const value of this.allocations.values()) total += value;
    return total;
  }
  reserve(key: string, bytes: number): boolean {
    if (
      !Number.isSafeInteger(bytes) ||
      bytes < 0 ||
      this.used - (this.allocations.get(key) ?? 0) + bytes > this.limit
    )
      return false;
    if (bytes) this.allocations.set(key, bytes);
    else this.allocations.delete(key);
    return true;
  }
  reserveMany(entries: [string, number][]): boolean {
    const next = new Map(this.allocations);
    for (const [key, bytes] of entries) {
      if (!Number.isSafeInteger(bytes) || bytes < 0) return false;
      if (bytes) next.set(key, bytes);
      else next.delete(key);
    }
    let total = 0;
    for (const bytes of next.values()) total += bytes;
    if (total > this.limit) return false;
    this.allocations.clear();
    for (const [key, bytes] of next) this.allocations.set(key, bytes);
    return true;
  }
  release(key: string) {
    this.allocations.delete(key);
  }
}
export const payloadBytes = (value: unknown) =>
  new TextEncoder().encode(JSON.stringify(value)).byteLength;
