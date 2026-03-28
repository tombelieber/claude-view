import { describe, expect, it } from 'bun:test'
import { statsTools } from '../tools/stats.js'

describe('statsTools', () => {
  it('exports get_stats tool definition', () => {
    const tool = statsTools.find((t) => t.name === 'get_stats')
    expect(tool).toBeDefined()
    expect(tool!.annotations.readOnlyHint).toBe(true)
  })

  it('exports get_fluency_score tool definition', () => {
    const tool = statsTools.find((t) => t.name === 'get_fluency_score')
    expect(tool).toBeDefined()
  })

  it('exports get_token_stats tool definition', () => {
    const tool = statsTools.find((t) => t.name === 'get_token_stats')
    expect(tool).toBeDefined()
  })

  it('exports exactly 3 tools', () => {
    expect(statsTools).toHaveLength(3)
  })
})
