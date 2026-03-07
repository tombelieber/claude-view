import { describe, expect, it } from 'bun:test'
import { sessionTools } from '../tools/sessions.js'

describe('sessionTools', () => {
  it('exports list_sessions tool definition', () => {
    const tool = sessionTools.find((t) => t.name === 'list_sessions')
    expect(tool).toBeDefined()
    expect(tool!.annotations.readOnlyHint).toBe(true)
  })

  it('exports get_session tool definition', () => {
    const tool = sessionTools.find((t) => t.name === 'get_session')
    expect(tool).toBeDefined()
    expect(tool!.inputSchema.shape.session_id).toBeDefined()
  })

  it('exports search_sessions tool definition', () => {
    const tool = sessionTools.find((t) => t.name === 'search_sessions')
    expect(tool).toBeDefined()
    expect(tool!.inputSchema.shape.query).toBeDefined()
  })

  it('exports exactly 3 tools', () => {
    expect(sessionTools).toHaveLength(3)
  })
})
