import { describe, expect, it } from 'vitest'
import { type MetaEvent, metaTransition } from './meta'

const initEvent: MetaEvent = {
  type: 'SESSION_INIT',
  model: 'opus-4',
  permissionMode: 'default',
  slashCommands: ['/help'],
  mcpServers: [{ name: 'ctx7', status: 'connected' }],
  skills: ['commit'],
  agents: [],
  capabilities: ['computer_use'],
}

describe('metaTransition', () => {
  it('SESSION_INIT creates meta from null', () => {
    const result = metaTransition(null, initEvent)
    expect(result).toEqual({
      model: 'opus-4',
      permissionMode: 'default',
      slashCommands: ['/help'],
      mcpServers: [{ name: 'ctx7', status: 'connected' }],
      skills: ['commit'],
      agents: [],
      capabilities: ['computer_use'],
      totalInputTokens: 0,
      contextWindowSize: 0,
    })
  })

  it('SERVER_MODE_CONFIRMED updates permissionMode', () => {
    const meta = metaTransition(null, initEvent)
    const result = metaTransition(meta, { type: 'SERVER_MODE_CONFIRMED', mode: 'plan' })
    expect(result).not.toBeNull()
    expect(result?.permissionMode).toBe('plan')
    expect(result?.model).toBe('opus-4') // unchanged
  })

  it('COMMANDS_UPDATED replaces slashCommands', () => {
    const meta = metaTransition(null, initEvent)
    const result = metaTransition(meta, {
      type: 'COMMANDS_UPDATED',
      commands: ['/commit', '/review'],
    })
    expect(result).not.toBeNull()
    expect(result?.slashCommands).toEqual(['/commit', '/review'])
  })

  it('AGENTS_UPDATED replaces agents', () => {
    const meta = metaTransition(null, initEvent)
    const result = metaTransition(meta, { type: 'AGENTS_UPDATED', agents: ['explore', 'code'] })
    expect(result).not.toBeNull()
    expect(result?.agents).toEqual(['explore', 'code'])
  })

  it('TURN_USAGE updates token counts', () => {
    const meta = metaTransition(null, initEvent)
    const result = metaTransition(meta, {
      type: 'TURN_USAGE',
      totalInputTokens: 5000,
      contextWindowSize: 200000,
    })
    expect(result).not.toBeNull()
    expect(result?.totalInputTokens).toBe(5000)
    expect(result?.contextWindowSize).toBe(200000)
  })

  it('event on null meta (non-INIT) returns null', () => {
    const result = metaTransition(null, { type: 'SERVER_MODE_CONFIRMED', mode: 'plan' })
    expect(result).toBeNull()
  })
})
