// sidecar/src/permission-handler.test.ts
import { describe, expect, it, vi } from 'vitest'
import { PermissionHandler } from './permission-handler.js'

describe('PermissionHandler', () => {
  it('routes AskUserQuestion to ask_question event', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'AskUserQuestion',
      { questions: [{ question: 'Pick one', header: 'H', options: [], multiSelect: false }] },
      { signal, toolUseID: 'tu_1' },
      emit,
    )

    // Should have emitted ask_question event
    expect(events).toHaveLength(1)
    expect((events[0] as Record<string, unknown>).type).toBe('ask_question')

    // Resolve the question
    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolveQuestion(requestId, { 'Pick one': 'Option A' })

    const result = await promise
    expect(result.behavior).toBe('allow')
  })

  it('routes generic tools to permission_request with full context', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'rm -rf /' },
      { signal, toolUseID: 'tu_2', decisionReason: 'dangerous command', blockedPath: '/' },
      emit,
    )

    expect(events).toHaveLength(1)
    const req = events[0] as Record<string, unknown>
    expect(req.type).toBe('permission_request')
    expect(req.toolUseID).toBe('tu_2')
    expect(req.decisionReason).toBe('dangerous command')
    expect(req.blockedPath).toBe('/')

    // Deny
    const requestId = req.requestId as string
    handler.resolvePermission(requestId, false)

    const result = await promise
    expect(result.behavior).toBe('deny')
  })

  it('auto-denies after timeout', async () => {
    vi.useFakeTimers()
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'ls' },
      { signal, toolUseID: 'tu_3' },
      emit,
      { timeoutMs: 1000 },
    )

    vi.advanceTimersByTime(1001)
    const result = await promise
    expect(result.behavior).toBe('deny')
    if (result.behavior === 'deny') {
      expect(result.message).toContain('timed out')
    }

    vi.useRealTimers()
  })

  it('drainAll denies all pending', () => {
    const handler = new PermissionHandler()
    const promises: Promise<unknown>[] = []
    const emit = () => {}
    const signal = new AbortController().signal

    promises.push(handler.handleCanUseTool('Bash', {}, { signal, toolUseID: '1' }, emit))
    promises.push(handler.handleCanUseTool('Edit', {}, { signal, toolUseID: '2' }, emit))

    handler.drainAll()
    // All should resolve
    return Promise.all(promises)
  })
})
