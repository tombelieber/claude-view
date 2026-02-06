import { describe, it, expect } from 'vitest'
import { buildThreadMap, getThreadChain } from './thread-map'

interface MockMsg {
  uuid?: string | null
  parent_uuid?: string | null
}

describe('buildThreadMap', () => {
  it('returns empty map for empty array', () => {
    expect(buildThreadMap([])).toEqual(new Map())
  })

  it('assigns indent 0 and isChild false to root messages', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: null },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('a')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
    expect(map.get('b')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
  })

  it('assigns indent 1 to direct children', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('b')).toEqual({ indent: 1, isChild: true, parentUuid: 'a' })
  })

  it('assigns incrementing indent to nested chains', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
      { uuid: 'c', parent_uuid: 'b' },
      { uuid: 'd', parent_uuid: 'c' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('a')!.indent).toBe(0)
    expect(map.get('b')!.indent).toBe(1)
    expect(map.get('c')!.indent).toBe(2)
    expect(map.get('d')!.indent).toBe(3)
  })

  it('caps indent at 5 (matching MAX_INDENT_LEVEL)', () => {
    const msgs: MockMsg[] = [
      { uuid: '0', parent_uuid: null },
      { uuid: '1', parent_uuid: '0' },
      { uuid: '2', parent_uuid: '1' },
      { uuid: '3', parent_uuid: '2' },
      { uuid: '4', parent_uuid: '3' },
      { uuid: '5', parent_uuid: '4' },
      { uuid: '6', parent_uuid: '5' },
      { uuid: '7', parent_uuid: '6' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('5')!.indent).toBe(5)
    expect(map.get('6')!.indent).toBe(5)
    expect(map.get('7')!.indent).toBe(5)
  })

  it('skips messages with null/undefined uuid', () => {
    const msgs: MockMsg[] = [
      { uuid: null, parent_uuid: null },
      { uuid: undefined, parent_uuid: null },
      { uuid: 'b', parent_uuid: null },
    ]
    const map = buildThreadMap(msgs)
    expect(map.size).toBe(1)
    expect(map.has('b')).toBe(true)
  })

  it('treats orphaned children (parent_uuid not in list) as root', () => {
    const msgs: MockMsg[] = [
      { uuid: 'b', parent_uuid: 'nonexistent' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('b')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
  })

  it('handles sibling branches correctly', () => {
    const msgs: MockMsg[] = [
      { uuid: 'root', parent_uuid: null },
      { uuid: 'child1', parent_uuid: 'root' },
      { uuid: 'child2', parent_uuid: 'root' },
      { uuid: 'grandchild1', parent_uuid: 'child1' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('child1')!.indent).toBe(1)
    expect(map.get('child2')!.indent).toBe(1)
    expect(map.get('grandchild1')!.indent).toBe(2)
  })

  it('handles messages with empty string uuid', () => {
    const msgs: MockMsg[] = [{ uuid: '', parent_uuid: null }]
    const map = buildThreadMap(msgs)
    expect(map.size).toBe(0)
  })

  it('handles parent_uuid pointing to self (cycle)', () => {
    const msgs: MockMsg[] = [{ uuid: 'a', parent_uuid: 'a' }]
    const map = buildThreadMap(msgs)
    expect(map.get('a')!.indent).toBe(0)
  })

  it('handles circular parent chains', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: 'b' },
      { uuid: 'b', parent_uuid: 'a' },
    ]
    const map = buildThreadMap(msgs)
    const indentA = map.get('a')!.indent
    const indentB = map.get('b')!.indent
    expect(indentA + indentB).toBeLessThanOrEqual(2)
  })

  it('performs acceptably with 1000 messages', () => {
    const msgs: MockMsg[] = Array.from({ length: 1000 }, (_, i) => ({
      uuid: String(i),
      parent_uuid: i > 0 ? String(i - 1) : null,
    }))
    const start = performance.now()
    const map = buildThreadMap(msgs)
    const elapsed = performance.now() - start
    expect(map.size).toBe(1000)
    expect(elapsed).toBeLessThan(500)
  })
})

describe('getThreadChain', () => {
  it('returns ancestors and descendants for a mid-chain node', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
      { uuid: 'c', parent_uuid: 'b' },
      { uuid: 'd', parent_uuid: 'b' },
    ]
    const chain = getThreadChain('b', msgs)
    expect(chain).toEqual(new Set(['a', 'b', 'c', 'd']))
  })

  it('returns just self for isolated root', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'x', parent_uuid: null },
    ]
    expect(getThreadChain('a', msgs)).toEqual(new Set(['a']))
  })

  it('returns full linear chain from leaf', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
      { uuid: 'c', parent_uuid: 'b' },
    ]
    expect(getThreadChain('c', msgs)).toEqual(new Set(['a', 'b', 'c']))
  })

  it('handles circular references without infinite loop', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: 'b' },
      { uuid: 'b', parent_uuid: 'a' },
    ]
    const chain = getThreadChain('a', msgs)
    expect(chain.has('a')).toBe(true)
    expect(chain.has('b')).toBe(true)
  })
})
