import { renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { SessionChannel } from '../../lib/session-channel'
import { NOOP_ACTIONS, useSessionActions } from '../use-session-actions'

describe('useSessionActions — two-pipe architecture', () => {
  // ─── user_message goes through `send` (can trigger resume) ──────────────
  describe('sendMessage uses resume-capable send', () => {
    it('sendMessage sends user_message via send pipe', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.sendMessage('hello')
      expect(send).toHaveBeenCalledWith({ type: 'user_message', content: 'hello' })
      expect(sendIfLive).not.toHaveBeenCalled()
    })
  })

  // ─── Control commands go through `sendIfLive` (live-only) ───────────────
  describe('control commands use live-only sendIfLive', () => {
    it('setPermissionMode uses sendIfLive, not send', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.setPermissionMode('bypassPermissions')
      expect(sendIfLive).toHaveBeenCalledWith({ type: 'set_mode', mode: 'bypassPermissions' })
      expect(send).not.toHaveBeenCalled()
    })

    it('interrupt uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.interrupt()
      expect(sendIfLive).toHaveBeenCalledWith({ type: 'interrupt' })
      expect(send).not.toHaveBeenCalled()
    })

    it('setModel uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.setModel('claude-opus-4-20250514')
      expect(sendIfLive).toHaveBeenCalledWith({
        type: 'set_model',
        model: 'claude-opus-4-20250514',
      })
      expect(send).not.toHaveBeenCalled()
    })

    it('setMaxThinkingTokens uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.setMaxThinkingTokens(4096)
      expect(sendIfLive).toHaveBeenCalledWith({
        type: 'set_max_thinking_tokens',
        maxThinkingTokens: 4096,
      })
      expect(send).not.toHaveBeenCalled()
    })

    it('stopTask uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.stopTask('task-42')
      expect(sendIfLive).toHaveBeenCalledWith({ type: 'stop_task', taskId: 'task-42' })
      expect(send).not.toHaveBeenCalled()
    })

    it('reconnectMcp uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.reconnectMcp('github')
      expect(sendIfLive).toHaveBeenCalledWith({ type: 'reconnect_mcp', serverName: 'github' })
      expect(send).not.toHaveBeenCalled()
    })

    it('toggleMcp uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.toggleMcp('slack', false)
      expect(sendIfLive).toHaveBeenCalledWith({
        type: 'toggle_mcp',
        serverName: 'slack',
        enabled: false,
      })
      expect(send).not.toHaveBeenCalled()
    })
  })

  // ─── Interactive responses go through sendIfLive (session is live) ──────
  describe('interactive responses use sendIfLive', () => {
    it('respondPermission uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.respondPermission('req-1', true, ['perm1'])
      expect(sendIfLive).toHaveBeenCalledWith({
        type: 'permission_response',
        requestId: 'req-1',
        allowed: true,
        updatedPermissions: ['perm1'],
      })
      expect(send).not.toHaveBeenCalled()
    })

    it('answerQuestion uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.answerQuestion('req-2', { q1: 'yes' })
      expect(sendIfLive).toHaveBeenCalledWith({
        type: 'question_response',
        requestId: 'req-2',
        answers: { q1: 'yes' },
      })
      expect(send).not.toHaveBeenCalled()
    })

    it('approvePlan uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.approvePlan('req-3', true, 'looks good')
      expect(sendIfLive).toHaveBeenCalledWith({
        type: 'plan_response',
        requestId: 'req-3',
        approved: true,
        feedback: 'looks good',
      })
      expect(send).not.toHaveBeenCalled()
    })

    it('submitElicitation uses sendIfLive', () => {
      const send = vi.fn()
      const sendIfLive = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, sendIfLive, null))

      result.current.submitElicitation('req-4', 'my response')
      expect(sendIfLive).toHaveBeenCalledWith({
        type: 'elicitation_response',
        requestId: 'req-4',
        response: 'my response',
      })
      expect(send).not.toHaveBeenCalled()
    })
  })

  // ─── Regression: control commands no-op when sendIfLive is null ─────────
  describe('control commands silently no-op when sendIfLive is null (dormant session)', () => {
    it('setPermissionMode does not call send when sendIfLive is null', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null, null))

      result.current.setPermissionMode('bypassPermissions')
      // Must NOT call send — that would trigger session resume
      expect(send).not.toHaveBeenCalled()
    })

    it('all control commands are safe no-ops when sendIfLive is null', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null, null))

      result.current.setPermissionMode('plan')
      result.current.interrupt()
      result.current.setModel('claude-opus-4-20250514')
      result.current.setMaxThinkingTokens(null)
      result.current.stopTask('t1')
      result.current.reconnectMcp('gh')
      result.current.toggleMcp('gh', true)
      result.current.respondPermission('r1', true)
      result.current.answerQuestion('r2', {})
      result.current.approvePlan('r3', true)
      result.current.submitElicitation('r4', 'x')
      // send MUST NOT be called for any of these
      expect(send).not.toHaveBeenCalled()
    })

    it('sendMessage STILL works when sendIfLive is null (triggers resume)', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null, null))

      result.current.sendMessage('hello')
      expect(send).toHaveBeenCalledWith({ type: 'user_message', content: 'hello' })
    })
  })

  // ─── Request/response (channel-based, already live-only) ────────────────
  describe('request/response methods via channel', () => {
    it('queryModels calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, send, channel))

      result.current.queryModels()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_models' })
    })

    it('queryCommands calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, send, channel))

      result.current.queryCommands()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_commands' })
    })

    it('queryAgents calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, send, channel))

      result.current.queryAgents()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_agents' })
    })

    it('queryMcpStatus calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, send, channel))

      result.current.queryMcpStatus()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_mcp_status' })
    })

    it('queryAccountInfo calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue({})
      const { result } = renderHook(() => useSessionActions(send, send, channel))

      result.current.queryAccountInfo()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_account_info' })
    })

    it('setMcpServers calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue({})
      const { result } = renderHook(() => useSessionActions(send, send, channel))

      result.current.setMcpServers({ gh: { command: 'gh' } })
      expect(requestSpy).toHaveBeenCalledWith({
        type: 'set_mcp_servers',
        servers: { gh: { command: 'gh' } },
      })
    })

    it('rewindFiles calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue({})
      const { result } = renderHook(() => useSessionActions(send, send, channel))

      result.current.rewindFiles('msg-1', { dryRun: true })
      expect(requestSpy).toHaveBeenCalledWith({
        type: 'rewind_files',
        userMessageId: 'msg-1',
        dryRun: true,
      })
    })
  })

  // ─── No session (send=null) → all NOOP ──────────────────────────────────
  describe('when send is null (no session)', () => {
    it('all fire-and-forget actions are no-ops', () => {
      const { result } = renderHook(() => useSessionActions(null, null, null))

      result.current.sendMessage('hello')
      result.current.respondPermission('req-1', true)
      result.current.answerQuestion('req-2', { q1: 'yes' })
      result.current.approvePlan('req-3', false)
      result.current.submitElicitation('req-4', 'x')
      result.current.setPermissionMode('default')
      result.current.interrupt()
      result.current.setModel('claude-opus-4-20250514')
      result.current.setMaxThinkingTokens(null)
      result.current.stopTask('t1')
      result.current.reconnectMcp('gh')
      result.current.toggleMcp('gh', true)
    })

    it('request/response methods return rejected promises', async () => {
      const { result } = renderHook(() => useSessionActions(null, null, null))

      await expect(result.current.queryModels()).rejects.toThrow('No session')
      await expect(result.current.queryCommands()).rejects.toThrow('No session')
      await expect(result.current.queryAgents()).rejects.toThrow('No session')
      await expect(result.current.queryMcpStatus()).rejects.toThrow('No session')
      await expect(result.current.queryAccountInfo()).rejects.toThrow('No session')
      await expect(result.current.setMcpServers({})).rejects.toThrow('No session')
      await expect(result.current.rewindFiles('m1')).rejects.toThrow('No session')
    })
  })

  // ─── NOOP_ACTIONS ───────────────────────────────────────────────────────
  describe('NOOP_ACTIONS', () => {
    it('fire-and-forget no-ops do not throw', () => {
      NOOP_ACTIONS.interrupt()
      NOOP_ACTIONS.setModel('x')
      NOOP_ACTIONS.setMaxThinkingTokens(null)
      NOOP_ACTIONS.stopTask('t1')
      NOOP_ACTIONS.reconnectMcp('gh')
      NOOP_ACTIONS.toggleMcp('gh', true)
    })

    it('request/response no-ops return rejected promises', async () => {
      await expect(NOOP_ACTIONS.queryModels()).rejects.toThrow('No session')
      await expect(NOOP_ACTIONS.queryCommands()).rejects.toThrow('No session')
      await expect(NOOP_ACTIONS.queryAgents()).rejects.toThrow('No session')
      await expect(NOOP_ACTIONS.queryMcpStatus()).rejects.toThrow('No session')
      await expect(NOOP_ACTIONS.queryAccountInfo()).rejects.toThrow('No session')
      await expect(NOOP_ACTIONS.setMcpServers({})).rejects.toThrow('No session')
      await expect(NOOP_ACTIONS.rewindFiles('m1')).rejects.toThrow('No session')
    })
  })

  // ─── Memoization ───────────────────────────────────────────────────────
  it('returns stable reference when deps do not change', () => {
    const send = vi.fn()
    const sendIfLive = vi.fn()
    const { result, rerender } = renderHook(() => useSessionActions(send, sendIfLive, null))
    const first = result.current
    rerender()
    expect(result.current).toBe(first)
  })
})
