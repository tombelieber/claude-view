import { renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useSessionCapabilities } from './use-session-capabilities'

describe('useSessionCapabilities', () => {
  it('returns capabilities from sessionInfo fields', () => {
    const sessionInfo = {
      isLive: true,
      sessionState: 'waiting_input',
      controlId: 'abc',

      totalInputTokens: 5000,
      contextWindowSize: 200000,
      model: 'claude-sonnet-4-6',
      slashCommands: ['commit', 'test'],
      mcpServers: [{ name: 'gh', status: 'connected' }],
      permissionMode: 'default',
      skills: [],
      agents: [],
    }
    const { result } = renderHook(() => useSessionCapabilities(sessionInfo))
    expect(result.current.model).toBe('claude-sonnet-4-6')
    expect(result.current.slashCommands).toEqual(['commit', 'test'])
    expect(result.current.mcpServers).toEqual([{ name: 'gh', status: 'connected' }])
    expect(result.current.permissionMode).toBe('default')
  })

  it('returns empty defaults when sessionInfo has no session data', () => {
    const sessionInfo = {
      isLive: false,
      sessionState: 'idle',
      controlId: null,

      totalInputTokens: 0,
      contextWindowSize: 0,
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
    }
    const { result } = renderHook(() => useSessionCapabilities(sessionInfo))
    expect(result.current.model).toBe('')
    expect(result.current.slashCommands).toEqual([])
    expect(result.current.mcpServers).toEqual([])
  })

  it('fastModeState defaults to undefined', () => {
    const sessionInfo = {
      isLive: true,
      sessionState: 'waiting_input',
      controlId: 'abc',

      totalInputTokens: 0,
      contextWindowSize: 0,
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
    }
    const { result } = renderHook(() => useSessionCapabilities(sessionInfo))
    expect(result.current.fastModeState).toBeUndefined()
  })
})
