import { renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { useSessionActions } from '../use-session-actions'

describe('useSessionActions', () => {
  describe('when send is provided', () => {
    it('sendMessage sends user_message with content', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send))

      result.current.sendMessage('hello')
      expect(send).toHaveBeenCalledWith({ type: 'user_message', content: 'hello' })
    })

    it('respondPermission sends permission_response', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send))

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
      const { result } = renderHook(() => useSessionActions(send))

      result.current.answerQuestion('req-2', { q1: 'yes' })
      expect(send).toHaveBeenCalledWith({
        type: 'question_response',
        requestId: 'req-2',
        answers: { q1: 'yes' },
      })
    })

    it('approvePlan sends plan_response', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send))

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
      const { result } = renderHook(() => useSessionActions(send))

      result.current.submitElicitation('req-4', 'my response')
      expect(send).toHaveBeenCalledWith({
        type: 'elicitation_response',
        requestId: 'req-4',
        response: 'my response',
      })
    })

    it('setPermissionMode sends set_mode', () => {
      const send = vi.fn()
      const { result } = renderHook(() => useSessionActions(send))

      result.current.setPermissionMode('bypassPermissions')
      expect(send).toHaveBeenCalledWith({ type: 'set_mode', mode: 'bypassPermissions' })
    })
  })

  describe('when send is null', () => {
    it('all actions are no-ops', () => {
      const { result } = renderHook(() => useSessionActions(null))

      // None of these should throw
      result.current.sendMessage('hello')
      result.current.respondPermission('req-1', true)
      result.current.answerQuestion('req-2', { q1: 'yes' })
      result.current.approvePlan('req-3', false)
      result.current.submitElicitation('req-4', 'x')
      result.current.setPermissionMode('default')
    })
  })

  it('returns stable reference when send does not change', () => {
    const send = vi.fn()
    const { result, rerender } = renderHook(() => useSessionActions(send))
    const first = result.current
    rerender()
    expect(result.current).toBe(first)
  })
})
