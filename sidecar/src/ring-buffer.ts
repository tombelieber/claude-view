// sidecar/src/ring-buffer.ts
export class RingBuffer<T> {
  private buffer: (T | undefined)[]
  private head = 0
  private size = 0

  constructor(private capacity: number) {
    this.buffer = new Array(capacity)
  }

  push(item: T): void {
    this.buffer[this.head] = item
    this.head = (this.head + 1) % this.capacity
    if (this.size < this.capacity) this.size++
  }

  toArray(): T[] {
    if (this.size === 0) return []
    const start = (this.head - this.size + this.capacity) % this.capacity
    const result: T[] = []
    for (let i = 0; i < this.size; i++) {
      // Safe: loop only iterates over populated slots (i < this.size)
      result.push(this.buffer[(start + i) % this.capacity] as T)
    }
    return result
  }

  /**
   * Get all items where keyFn(item) > threshold, in insertion order.
   * Returns null if the buffer has been evicted past the threshold
   * (caller was offline too long — replay is impossible).
   */
  getAfter(threshold: number, keyFn: (item: T) => number): T[] | null {
    const items = this.toArray()
    if (items.length === 0) return threshold < 0 ? [] : null

    // threshold < 0 means "give me everything" — initial sync, no gap check needed
    if (threshold < 0) return items

    // Check if we can serve the request — oldest item must have seq <= threshold + 1
    const oldestSeq = keyFn(items[0])
    if (oldestSeq > threshold + 1) return null // gap — replay impossible

    return items.filter((item) => keyFn(item) > threshold)
  }

  get length(): number {
    return this.size
  }

  clear(): void {
    this.buffer = new Array(this.capacity)
    this.head = 0
    this.size = 0
  }
}
