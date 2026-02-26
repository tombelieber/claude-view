import { describe, it, expect } from 'vitest'
import { buildThreadMap } from '../lib/thread-map'

describe('ConversationView thread integration', () => {
  it('computes correct indent for a typical tool-call conversation', () => {
    const messages = [
      { uuid: 'u1', parent_uuid: null, role: 'user', content: 'Fix the bug' },
      { uuid: 'a1', parent_uuid: 'u1', role: 'assistant', content: 'Let me look...' },
      { uuid: 't1', parent_uuid: 'a1', role: 'tool_use', content: '' },
      { uuid: 'r1', parent_uuid: 't1', role: 'tool_result', content: '' },
      { uuid: 'a2', parent_uuid: 'r1', role: 'assistant', content: 'Fixed.' },
    ]
    const map = buildThreadMap(messages)

    expect(map.get('u1')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
    expect(map.get('a1')).toEqual({ indent: 1, isChild: true, parentUuid: 'u1' })
    expect(map.get('t1')).toEqual({ indent: 2, isChild: true, parentUuid: 'a1' })
    expect(map.get('r1')).toEqual({ indent: 3, isChild: true, parentUuid: 't1' })
    expect(map.get('a2')).toEqual({ indent: 4, isChild: true, parentUuid: 'r1' })
  })

  it('handles compact mode (only user + assistant, orphaned from parents)', () => {
    const compactMessages = [
      { uuid: 'u1', parent_uuid: null, role: 'user', content: 'Fix the bug' },
      { uuid: 'a2', parent_uuid: 'r1', role: 'assistant', content: 'Fixed.' },
    ]
    const map = buildThreadMap(compactMessages)

    expect(map.get('u1')!.indent).toBe(0)
    expect(map.get('a2')!.indent).toBe(0)
    expect(map.get('a2')!.isChild).toBe(false)
  })

  it('handles messages with no uuid at all', () => {
    const messages = [
      { uuid: null, parent_uuid: null, role: 'system', content: '' },
      { uuid: 'a1', parent_uuid: null, role: 'assistant', content: 'hi' },
    ]
    const map = buildThreadMap(messages)
    expect(map.size).toBe(1)
  })
})
