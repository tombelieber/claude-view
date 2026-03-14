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

  it('routes ExitPlanMode to plan_approval event', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'ExitPlanMode',
      { allowedPrompts: ['Implement the plan'] },
      { signal, toolUseID: 'tu_plan' },
      emit,
    )

    expect(events).toHaveLength(1)
    expect((events[0] as Record<string, unknown>).type).toBe('plan_approval')
    const ev = events[0] as Record<string, unknown>
    expect((ev.planData as Record<string, unknown>).allowedPrompts).toBeDefined()

    const requestId = ev.requestId as string
    handler.resolvePlan(requestId, true)

    const result = await promise
    expect(result.behavior).toBe('allow')
  })

  it('routes ExitPlanMode to plan_approval and deny resolves with deny', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'ExitPlanMode',
      {},
      { signal, toolUseID: 'tu_plan_deny' },
      emit,
    )

    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolvePlan(requestId, false, 'Not ready')

    const result = await promise
    expect(result.behavior).toBe('deny')
    if (result.behavior === 'deny') {
      expect(result.message).toBe('Not ready')
    }
  })

  it('routes MCP elicitation tools (non-standard, with prompt field) to elicitation event', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'mcp__my_server__gather_info', // non-standard tool name
      { prompt: 'What is your GitHub username?' },
      { signal, toolUseID: 'tu_elicit' },
      emit,
    )

    expect(events).toHaveLength(1)
    const ev = events[0] as Record<string, unknown>
    expect(ev.type).toBe('elicitation')
    expect(ev.prompt).toBe('What is your GitHub username?')

    const requestId = ev.requestId as string
    handler.resolveElicitation(requestId, 'myuser')

    const result = await promise
    expect(result.behavior).toBe('allow')
  })

  // ─── SDK Zod schema compliance: all allow results must include updatedInput ───

  it('resolvePermission(allow) includes updatedInput for SDK Zod compliance', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    const promise = handler.handleCanUseTool(
      'WebSearch',
      { query: 'test' },
      { signal, toolUseID: 'tu_zod_1' },
      emit,
    )

    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolvePermission(requestId, true)

    const result = await promise
    expect(result.behavior).toBe('allow')
    if (result.behavior === 'allow') {
      // Must pass through the original input — NOT empty object!
      // Empty {} would cause tool to receive undefined for all fields.
      expect(result.updatedInput).toEqual({ query: 'test' })
    }
  })

  it('resolvePermission(deny) includes message for SDK Zod compliance', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'test' },
      { signal, toolUseID: 'tu_zod_2' },
      emit,
    )

    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolvePermission(requestId, false)

    const result = await promise
    expect(result.behavior).toBe('deny')
    if (result.behavior === 'deny') {
      expect(typeof result.message).toBe('string')
      expect(result.message!.length).toBeGreaterThan(0)
    }
  })

  it('resolvePlan(approve) passes through original input for SDK Zod compliance', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    const promise = handler.handleCanUseTool(
      'ExitPlanMode',
      { plan: 'do things' },
      { signal, toolUseID: 'tu_zod_3' },
      emit,
    )

    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolvePlan(requestId, true)

    const result = await promise
    expect(result.behavior).toBe('allow')
    if (result.behavior === 'allow') {
      expect(result.updatedInput).toEqual({ plan: 'do things' })
    }
  })

  it('resolveQuestion includes updatedInput with answers', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    const promise = handler.handleCanUseTool(
      'AskUserQuestion',
      { questions: [{ question: 'Color?', header: 'H', options: [], multiSelect: false }] },
      { signal, toolUseID: 'tu_zod_4' },
      emit,
    )

    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolveQuestion(requestId, { 'Color?': 'Blue' })

    const result = await promise
    expect(result.behavior).toBe('allow')
    if (result.behavior === 'allow') {
      expect(result.updatedInput).toEqual({ answers: { 'Color?': 'Blue' } })
    }
  })

  it('resolveElicitation includes updatedInput with response', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    const promise = handler.handleCanUseTool(
      'mcp__test__prompt',
      { prompt: 'Enter name:' },
      { signal, toolUseID: 'tu_zod_5' },
      emit,
    )

    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolveElicitation(requestId, 'Alice')

    const result = await promise
    expect(result.behavior).toBe('allow')
    if (result.behavior === 'allow') {
      expect(result.updatedInput).toEqual({ response: 'Alice' })
    }
  })

  it('timeout includes message for SDK Zod compliance', async () => {
    vi.useFakeTimers()
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'test' },
      { signal, toolUseID: 'tu_zod_6' },
      emit,
      { timeoutMs: 500 },
    )

    vi.advanceTimersByTime(501)
    const result = await promise
    expect(result.behavior).toBe('deny')
    if (result.behavior === 'deny') {
      expect(typeof result.message).toBe('string')
      expect(result.message!.length).toBeGreaterThan(0)
    }
    vi.useRealTimers()
  })

  it('resolvePermission passes updatedPermissions through when provided', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'ls' },
      { signal, toolUseID: 'tu_perm_up' },
      emit,
    )

    const requestId = (events[0] as Record<string, string>).requestId
    const perms = [{ tool: 'Bash', permission: 'allow' as const }]
    handler.resolvePermission(requestId, true, perms as never)

    const result = await promise
    expect(result.behavior).toBe('allow')
    if (result.behavior === 'allow') {
      expect(result.updatedInput).toEqual({ command: 'ls' })
      expect(result.updatedPermissions).toBe(perms)
    }
  })

  it('duplicate resolvePermission returns false (idempotent)', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const signal = new AbortController().signal

    handler.handleCanUseTool('Bash', { command: 'ls' }, { signal, toolUseID: 'tu_dup' }, emit)

    const requestId = (events[0] as Record<string, string>).requestId
    expect(handler.resolvePermission(requestId, true)).toBe(true)
    expect(handler.resolvePermission(requestId, true)).toBe(false) // already resolved
  })

  it('abort signal produces valid deny with message', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }
    const ac = new AbortController()

    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'ls' },
      { signal: ac.signal, toolUseID: 'tu_abort' },
      emit,
    )

    ac.abort()
    const result = await promise
    expect(result.behavior).toBe('deny')
    if (result.behavior === 'deny') {
      expect(typeof result.message).toBe('string')
      expect(result.message?.length).toBeGreaterThan(0)
    }
  })

  it('drainAll produces valid PermissionResult shapes for all types', async () => {
    const handler = new PermissionHandler()
    const emit = () => {}
    const signal = new AbortController().signal

    const permPromise = handler.handleCanUseTool(
      'Bash',
      { cmd: 'x' },
      { signal, toolUseID: '1' },
      emit,
    )
    const questionPromise = handler.handleCanUseTool(
      'AskUserQuestion',
      { questions: [{ question: 'Q', header: 'H', options: [], multiSelect: false }] },
      { signal, toolUseID: '2' },
      emit,
    )
    const planPromise = handler.handleCanUseTool(
      'ExitPlanMode',
      { p: 1 },
      { signal, toolUseID: '3' },
      emit,
    )
    const elicitPromise = handler.handleCanUseTool(
      'mcp__test__elicit',
      { prompt: 'Enter:' },
      { signal, toolUseID: '4' },
      emit,
    )

    handler.drainAll()

    const [perm, question, plan, elicit] = await Promise.all([
      permPromise,
      questionPromise,
      planPromise,
      elicitPromise,
    ])

    // Permission: deny with message
    expect(perm.behavior).toBe('deny')
    if (perm.behavior === 'deny') expect(typeof perm.message).toBe('string')

    // Question: allow with updatedInput
    expect(question.behavior).toBe('allow')
    if (question.behavior === 'allow') expect(question.updatedInput).toBeDefined()

    // Plan: deny with message
    expect(plan.behavior).toBe('deny')
    if (plan.behavior === 'deny') expect(typeof plan.message).toBe('string')

    // Elicitation: allow with updatedInput
    expect(elicit.behavior).toBe('allow')
    if (elicit.behavior === 'allow') expect(elicit.updatedInput).toBeDefined()
  })

  it('drainInteractive produces valid shapes', async () => {
    const handler = new PermissionHandler()
    const emit = () => {}
    const signal = new AbortController().signal

    const questionPromise = handler.handleCanUseTool(
      'AskUserQuestion',
      { questions: [{ question: 'Q', header: 'H', options: [], multiSelect: false }] },
      { signal, toolUseID: '5' },
      emit,
    )
    const planPromise = handler.handleCanUseTool(
      'ExitPlanMode',
      { p: 1 },
      { signal, toolUseID: '6' },
      emit,
    )
    const elicitPromise = handler.handleCanUseTool(
      'mcp__test__elicit2',
      { prompt: 'Name?' },
      { signal, toolUseID: '7' },
      emit,
    )

    handler.drainInteractive()

    const [question, plan, elicit] = await Promise.all([
      questionPromise,
      planPromise,
      elicitPromise,
    ])

    expect(question.behavior).toBe('allow')
    if (question.behavior === 'allow') expect(question.updatedInput).toBeDefined()

    expect(plan.behavior).toBe('deny')
    if (plan.behavior === 'deny') expect(typeof plan.message).toBe('string')

    expect(elicit.behavior).toBe('allow')
    if (elicit.behavior === 'allow') expect(elicit.updatedInput).toBeDefined()
  })

  it('does NOT route standard tools with prompt field to elicitation (heuristic boundary)', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => {
      events.push(e)
    }

    // Bash with a 'prompt' field in input should still go to permission_request, not elicitation
    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'echo hi', prompt: 'some prompt string' },
      { signal, toolUseID: 'tu_bash_prompt' },
      emit,
    )

    expect(events).toHaveLength(1)
    expect((events[0] as Record<string, unknown>).type).toBe('permission_request')

    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolvePermission(requestId, true)
    const result = await promise
    expect(result.behavior).toBe('allow')
  })
})
