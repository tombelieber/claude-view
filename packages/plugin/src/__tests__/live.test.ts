import { describe, expect, it } from 'bun:test'
import { liveTools } from '../tools/live.js'

describe('liveTools', () => {
  it('exports list_live_sessions tool definition', () => {
    const tool = liveTools.find((t) => t.name === 'list_live_sessions')
    expect(tool).toBeDefined()
    expect(tool!.annotations.readOnlyHint).toBe(true)
  })

  it('exports get_live_summary tool definition', () => {
    const tool = liveTools.find((t) => t.name === 'get_live_summary')
    expect(tool).toBeDefined()
  })

  it('exports exactly 2 tools', () => {
    expect(liveTools).toHaveLength(2)
  })
})
