import { describe, it, expect } from 'vitest'
import { computeCategoryCounts } from './compute-category-counts'
import type { RichMessage } from '../components/live/RichPane'

describe('computeCategoryCounts', () => {
  it('returns zero counts for empty array', () => {
    const counts = computeCategoryCounts([])
    expect(counts.builtin).toBe(0)
    expect(counts.hook).toBe(0)
    expect(counts.hook_progress).toBe(0)
    expect(counts.system).toBe(0)
  })

  it('counts each category from RichMessage array', () => {
    const messages: RichMessage[] = [
      { type: 'user', content: 'hi' },
      { type: 'tool_use', content: '', category: 'builtin' },
      { type: 'tool_use', content: '', category: 'builtin' },
      { type: 'tool_use', content: '', category: 'skill' },
      { type: 'tool_result', content: 'ok', category: 'builtin' },
      { type: 'system', content: '', category: 'system' },
      { type: 'progress', content: '', category: 'hook_progress' },
      { type: 'error', content: 'fail', category: 'error' },
      { type: 'summary', content: 'summary', category: 'summary' },
    ]
    const counts = computeCategoryCounts(messages)
    expect(counts.builtin).toBe(3) // 2 tool_use + 1 tool_result
    expect(counts.skill).toBe(1)
    expect(counts.system).toBe(1)
    expect(counts.summary).toBe(1)
    expect(counts.hook_progress).toBe(1)
    expect(counts.error).toBe(1)
  })

  it('ignores messages without a category', () => {
    const messages: RichMessage[] = [
      { type: 'user', content: 'hi' },
      { type: 'assistant', content: 'hello' },
      { type: 'thinking', content: 'hmm' },
    ]
    const counts = computeCategoryCounts(messages)
    const total = Object.values(counts).reduce((a, b) => a + b, 0)
    expect(total).toBe(0)
  })
})
