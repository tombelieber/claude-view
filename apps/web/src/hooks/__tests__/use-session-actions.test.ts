import { renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { SessionChannel } from '../../lib/session-channel'
import { NOOP_ACTIONS, useSessionActions } from '../use-session-actions'

describe('useSessionActions', () => {
  describe('when send is provided (existing 6 methods)', () => {
    it('sendMessage sends user_message with content', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))

      result.current.sendMessage('hello')
      expect(send).toHaveBeenCalledWith({ type: 'user_message', content: 'hello' })
    })

    it('respondPermission sends permission_response', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))

      result.current.respondPermission('req-1', true, ['perm1'])
      expect(send).toHaveBeenCalledWith({
        type: 'permission_response',
        requestId: 'req-1',
        allowed: true,
        updatedPermissions: ['perm1'],
      })
    })

    it('answerQuestion sends question_response', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))

      result.current.answerQuestion('req-2', { q1: 'yes' })
      expect(send).toHaveBeenCalledWith({
        type: 'question_response',
        requestId: 'req-2',
        answers: { q1: 'yes' },
      })
    })

    it('approvePlan sends plan_response', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))

      result.current.approvePlan('req-3', true, 'looks good')
      expect(send).toHaveBeenCalledWith({
        type: 'plan_response',
        requestId: 'req-3',
        approved: true,
        feedback: 'looks good',
      })
    })

    it('submitElicitation sends elicitation_response', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))

      result.current.submitElicitation('req-4', 'my response')
      expect(send).toHaveBeenCalledWith({
        type: 'elicitation_response',
        requestId: 'req-4',
        response: 'my response',
      })
    })

    it('setPermissionMode sends set_mode', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))

      result.current.setPermissionMode('bypassPermissions')
      expect(send).toHaveBeenCalledWith({ type: 'set_mode', mode: 'bypassPermissions' })
    })
  })

  describe('new fire-and-forget methods', () => {
    it('interrupt sends { type: "interrupt" }', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))
      result.current.interrupt()
      expect(send).toHaveBeenCalledWith({ type: 'interrupt' })
    })

    it('setModel sends { type: "set_model", model }', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))
      result.current.setModel('claude-opus-4-20250514')
      expect(send).toHaveBeenCalledWith({ type: 'set_model', model: 'claude-opus-4-20250514' })
    })

    it('setMaxThinkingTokens sends { type: "set_max_thinking_tokens", maxThinkingTokens }', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))
      result.current.setMaxThinkingTokens(4096)
      expect(send).toHaveBeenCalledWith({
        type: 'set_max_thinking_tokens',
        maxThinkingTokens: 4096,
      })
    })

    it('setMaxThinkingTokens sends null for default', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))
      result.current.setMaxThinkingTokens(null)
      expect(send).toHaveBeenCalledWith({
        type: 'set_max_thinking_tokens',
        maxThinkingTokens: null,
      })
    })

    it('stopTask sends { type: "stop_task", taskId }', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))
      result.current.stopTask('task-42')
      expect(send).toHaveBeenCalledWith({ type: 'stop_task', taskId: 'task-42' })
    })

    it('reconnectMcp sends { type: "reconnect_mcp", serverName }', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))
      result.current.reconnectMcp('github')
      expect(send).toHaveBeenCalledWith({ type: 'reconnect_mcp', serverName: 'github' })
    })

    it('toggleMcp sends { type: "toggle_mcp", serverName, enabled }', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send, null))
      result.current.toggleMcp('slack', false)
      expect(send).toHaveBeenCalledWith({
        type: 'toggle_mcp',
        serverName: 'slack',
        enabled: false,
      })
    })
  })

  describe('new request/response methods', () => {
    it('queryModels calls channel.request with { type: "query_models" }', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, channel))

      result.current.queryModels()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_models' })
    })

    it('queryCommands calls channel.request with { type: "query_commands" }', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, channel))

      result.current.queryCommands()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_commands' })
    })

    it('queryAgents calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, channel))

      result.current.queryAgents()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_agents' })
    })

    it('queryMcpStatus calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue([])
      const { result } = renderHook(() => useSessionActions(send, channel))

      result.current.queryMcpStatus()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_mcp_status' })
    })

    it('queryAccountInfo calls channel.request', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue({})
      const { result } = renderHook(() => useSessionActions(send, channel))

      result.current.queryAccountInfo()
      expect(requestSpy).toHaveBeenCalledWith({ type: 'query_account_info' })
    })

    it('setMcpServers calls channel.request with { type: "set_mcp_servers", servers }', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue({})
      const { result } = renderHook(() => useSessionActions(send, channel))

      result.current.setMcpServers({ gh: { command: 'gh' } })
      expect(requestSpy).toHaveBeenCalledWith({
        type: 'set_mcp_servers',
        servers: { gh: { command: 'gh' } },
      })
    })

    it('rewindFiles calls channel.request with { type: "rewind_files", userMessageId, dryRun }', () => {
      const send = vi.fn()
      const channel = new SessionChannel(vi.fn())
      const requestSpy = vi.spyOn(channel, 'request').mockResolvedValue({})
      const { result } = renderHook(() => useSessionActions(send, channel))

      result.current.rewindFiles('msg-1', { dryRun: true })
      expect(requestSpy).toHaveBeenCalledWith({
        type: 'rewind_files',
        userMessageId: 'msg-1',
        dryRun: true,
      })
    })
  })

  describe('when send is null', () => {
    it('all fire-and-forget actions are no-ops', () => {
      const { result } = renderHook(() => useSessionActions(null, null))

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
      const { result } = renderHook(() => useSessionActions(null, null))

      await expect(result.current.queryModels()).rejects.toThrow('No session')
      await expect(result.current.queryCommands()).rejects.toThrow('No session')
      await expect(result.current.queryAgents()).rejects.toThrow('No session')
      await expect(result.current.queryMcpStatus()).rejects.toThrow('No session')
      await expect(result.current.queryAccountInfo()).rejects.toThrow('No session')
      await expect(result.current.setMcpServers({})).rejects.toThrow('No session')
      await expect(result.current.rewindFiles('m1')).rejects.toThrow('No session')
    })
  })

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

  it('returns stable reference when send does not change', () => {
    const send = vi.fn()
    const { result, rerender } = renderHook(() => useSessionActions(send, null))
    const first = result.current
    rerender()
    expect(result.current).toBe(first)
  })
})
