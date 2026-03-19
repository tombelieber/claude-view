import { describe, expect, it } from 'vitest'
import { topologicalSort } from './dag-sort.js'

describe('topologicalSort', () => {
  it('sorts linear chain', () => {
    expect(
      topologicalSort(
        ['a', 'b', 'c'],
        [
          { source: 'a', target: 'b' },
          { source: 'b', target: 'c' },
        ],
      ),
    ).toEqual(['a', 'b', 'c'])
  })

  it('sorts parallel branches', () => {
    const result = topologicalSort(
      ['a', 'b', 'c', 'd'],
      [
        { source: 'a', target: 'b' },
        { source: 'a', target: 'c' },
        { source: 'b', target: 'd' },
        { source: 'c', target: 'd' },
      ],
    )
    expect(result.indexOf('a')).toBeLessThan(result.indexOf('b'))
    expect(result.indexOf('a')).toBeLessThan(result.indexOf('c'))
    expect(result.indexOf('b')).toBeLessThan(result.indexOf('d'))
    expect(result.indexOf('c')).toBeLessThan(result.indexOf('d'))
  })

  it('throws on cycle', () => {
    expect(() =>
      topologicalSort(
        ['a', 'b'],
        [
          { source: 'a', target: 'b' },
          { source: 'b', target: 'a' },
        ],
      ),
    ).toThrow('Cycle detected')
  })

  // Phase 1 constraint: only linear/converging DAGs. Parallel disconnected
  // subgraphs are Phase 2. Two unconnected stage nodes are invalid in Phase 1.
  it('throws on disconnected subgraph (Phase 1: no parallel subgraphs)', () => {
    expect(() => topologicalSort(['a', 'b'], [])).toThrow('Disconnected')
  })

  it('handles single node', () => {
    expect(topologicalSort(['a'], [])).toEqual(['a'])
  })

  it('filters self-loops', () => {
    expect(
      topologicalSort(
        ['a', 'b'],
        [
          { source: 'a', target: 'b' },
          { source: 'a', target: 'a' },
        ],
      ),
    ).toEqual(['a', 'b'])
  })
})
