// sidecar/src/ring-buffer.test.ts
import { describe, expect, it } from 'vitest'
import { RingBuffer } from './ring-buffer.js'

describe('RingBuffer', () => {
  it('stores and retrieves all items via toArray()', () => {
    const buf = new RingBuffer<{ seq: number }>(5)
    buf.push({ seq: 1 })
    buf.push({ seq: 2 })
    buf.push({ seq: 3 })
    expect(buf.toArray()).toEqual([{ seq: 1 }, { seq: 2 }, { seq: 3 }])
  })

  it('getAfter(-1) returns all items', () => {
    const buf = new RingBuffer<{ seq: number }>(5)
    buf.push({ seq: 1 })
    buf.push({ seq: 2 })
    buf.push({ seq: 3 })
    expect(buf.getAfter(-1, (item) => item.seq)).toEqual([{ seq: 1 }, { seq: 2 }, { seq: 3 }])
  })

  it('evicts oldest when full', () => {
    const buf = new RingBuffer<{ seq: number }>(3)
    buf.push({ seq: 1 })
    buf.push({ seq: 2 })
    buf.push({ seq: 3 })
    buf.push({ seq: 4 }) // evicts seq:1
    const items = buf.toArray()
    expect(items).toEqual([{ seq: 2 }, { seq: 3 }, { seq: 4 }])
  })

  it('getAfter returns items with seq > threshold', () => {
    const buf = new RingBuffer<{ seq: number; data: string }>(10)
    buf.push({ seq: 1, data: 'a' })
    buf.push({ seq: 2, data: 'b' })
    buf.push({ seq: 3, data: 'c' })
    const result = buf.getAfter(1, (item) => item.seq)
    expect(result).toEqual([
      { seq: 2, data: 'b' },
      { seq: 3, data: 'c' },
    ])
  })

  it('getAfter returns null when threshold is too old', () => {
    const buf = new RingBuffer<{ seq: number }>(3)
    buf.push({ seq: 5 })
    buf.push({ seq: 6 })
    buf.push({ seq: 7 })
    // seq 3 was evicted, buffer starts at 5
    const result = buf.getAfter(3, (item) => item.seq)
    expect(result).toBeNull()
  })

  it('getAfter returns empty array when threshold is current', () => {
    const buf = new RingBuffer<{ seq: number }>(5)
    buf.push({ seq: 1 })
    buf.push({ seq: 2 })
    const result = buf.getAfter(2, (item) => item.seq)
    expect(result).toEqual([])
  })
})
