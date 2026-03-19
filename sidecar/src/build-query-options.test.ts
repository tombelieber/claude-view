// sidecar/src/build-query-options.test.ts
// Tests for buildQueryOptions (private function) — verified indirectly via
// createControlSession and forkControlSession which pass options to SDK query().

import type { Options } from '@anthropic-ai/claude-agent-sdk'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SessionRegistry } from './session-registry.js'

// Capture the Options passed to the SDK query() function
let capturedOptions: Options | undefined

vi.mock('@anthropic-ai/claude-agent-sdk', () => ({
  query: vi.fn((args: { options: Options }) => {
    capturedOptions = args.options
    // Return a mock Query (async iterable that blocks forever)
    let blockResolve: (() => void) | null = null
    return {
      [Symbol.asyncIterator]() {
        return this
      },
      async next() {
        return new Promise<IteratorResult<unknown>>((resolve) => {
          blockResolve = () => resolve({ done: true, value: undefined })
        })
      },
      close: vi.fn(() => blockResolve?.()),
      return: vi.fn().mockResolvedValue({ done: true, value: undefined }),
      throw: vi.fn().mockResolvedValue({ done: true, value: undefined }),
      interrupt: vi.fn().mockResolvedValue(undefined),
      setPermissionMode: vi.fn().mockResolvedValue(undefined),
      setModel: vi.fn().mockResolvedValue(undefined),
      setMaxThinkingTokens: vi.fn().mockResolvedValue(undefined),
      supportedModels: vi.fn().mockResolvedValue([]),
      supportedCommands: vi.fn().mockResolvedValue([]),
      supportedAgents: vi.fn().mockResolvedValue([]),
      mcpServerStatus: vi.fn().mockResolvedValue([]),
      accountInfo: vi.fn().mockResolvedValue({}),
      rewindFiles: vi.fn().mockResolvedValue({}),
      reconnectMcpServer: vi.fn().mockResolvedValue(undefined),
      toggleMcpServer: vi.fn().mockResolvedValue(undefined),
      setMcpServers: vi.fn().mockResolvedValue({}),
      stopTask: vi.fn().mockResolvedValue(undefined),
      streamInput: vi.fn().mockResolvedValue(undefined),
      initializationResult: vi.fn().mockResolvedValue({}),
    }
  }),
  listSessions: vi.fn().mockResolvedValue([]),
}))

vi.mock('./cli-path.js', () => ({
  findClaudeExecutable: vi.fn().mockReturnValue('/usr/local/bin/claude'),
}))

// Import AFTER mocks
const { createControlSession, forkControlSession } = await import('./sdk-session.js')

describe('buildQueryOptions (via createControlSession)', () => {
  let registry: SessionRegistry

  beforeEach(() => {
    registry = new SessionRegistry()
    capturedOptions = undefined
  })

  it('always includes settingSources: [user, project]', () => {
    createControlSession({ model: 'claude-haiku-4-5-20251001' }, registry)

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.settingSources).toEqual(['user', 'project'])
  })

  it('sets allowDangerouslySkipPermissions when bypassPermissions mode', () => {
    createControlSession(
      { model: 'claude-haiku-4-5-20251001', permissionMode: 'bypassPermissions' },
      registry,
    )

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.permissionMode).toBe('bypassPermissions')
    // biome-ignore lint/suspicious/noExplicitAny: testing private SDK option
    expect((capturedOptions as any).allowDangerouslySkipPermissions).toBe(true)
  })

  it('does NOT set allowDangerouslySkipPermissions for other modes', () => {
    for (const mode of ['default', 'plan', 'auto', 'acceptEdits']) {
      capturedOptions = undefined
      createControlSession({ model: 'claude-haiku-4-5-20251001', permissionMode: mode }, registry)

      expect(capturedOptions).toBeDefined()
      expect(capturedOptions!.permissionMode).toBe(mode)
      // biome-ignore lint/suspicious/noExplicitAny: testing private SDK option
      expect((capturedOptions as any).allowDangerouslySkipPermissions).toBeUndefined()
    }
  })

  it('defaults cwd to process.cwd() when no projectPath', () => {
    createControlSession({ model: 'claude-haiku-4-5-20251001' }, registry)

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.cwd).toBe(process.cwd())
  })

  it('passes projectPath as cwd when provided', () => {
    createControlSession(
      { model: 'claude-haiku-4-5-20251001', projectPath: '/Users/test/my-project' },
      registry,
    )

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.cwd).toBe('/Users/test/my-project')
  })
})

describe('buildQueryOptions (via forkControlSession)', () => {
  let registry: SessionRegistry

  beforeEach(() => {
    registry = new SessionRegistry()
    capturedOptions = undefined
  })

  it('passes projectPath for fork sessions', () => {
    forkControlSession(
      {
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
        model: 'claude-haiku-4-5-20251001',
        projectPath: '/Users/test/forked-project',
      },
      registry,
    )

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.cwd).toBe('/Users/test/forked-project')
    // biome-ignore lint/suspicious/noExplicitAny: testing private SDK option
    expect((capturedOptions as any).forkSession).toBe(true)
    expect(capturedOptions!.resume).toBe('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
  })

  it('always includes settingSources for fork sessions', () => {
    forkControlSession(
      {
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      },
      registry,
    )

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.settingSources).toEqual(['user', 'project'])
  })
})

describe('workflow engine fields (via createControlSession)', () => {
  let registry: SessionRegistry

  beforeEach(() => {
    registry = new SessionRegistry()
    capturedOptions = undefined
  })

  it('forwards effort when provided', () => {
    createControlSession({ model: 'claude-haiku-4-5-20251001', effort: 'high' }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).effort).toBe('high')
  })

  it('forwards maxBudgetUsd when provided', () => {
    createControlSession({ model: 'claude-haiku-4-5-20251001', maxBudgetUsd: 5.0 }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).maxBudgetUsd).toBe(5.0)
  })

  it('forwards maxTurns when provided', () => {
    createControlSession({ model: 'claude-haiku-4-5-20251001', maxTurns: 10 }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).maxTurns).toBe(10)
  })

  it('forwards systemPrompt string when provided', () => {
    createControlSession(
      { model: 'claude-haiku-4-5-20251001', systemPrompt: 'You are a code reviewer.' },
      registry,
    )

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).systemPrompt).toBe('You are a code reviewer.')
  })

  it('forwards systemPrompt preset with append', () => {
    const preset = {
      type: 'preset' as const,
      preset: 'claude_code' as const,
      append: 'Be concise.',
    }
    createControlSession({ model: 'claude-haiku-4-5-20251001', systemPrompt: preset }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).systemPrompt).toEqual(preset)
  })

  it('forwards outputFormat when provided', () => {
    const format = {
      type: 'json_schema' as const,
      schema: { type: 'object', properties: { result: { type: 'string' } } },
    }
    createControlSession({ model: 'claude-haiku-4-5-20251001', outputFormat: format }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).outputFormat).toEqual(format)
  })

  it('forwards plugins when provided', () => {
    const plugins = [{ type: 'local' as const, path: '/path/to/plugin' }]
    createControlSession({ model: 'claude-haiku-4-5-20251001', plugins }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).plugins).toEqual(plugins)
  })

  it('forwards hooks when provided', () => {
    const hooks = { PreToolUse: [{ matcher: '.*', command: 'echo ok' }] }
    createControlSession({ model: 'claude-haiku-4-5-20251001', hooks }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).hooks).toEqual(hooks)
  })

  it('forwards agents record when provided', () => {
    const agents = { reviewer: { model: 'claude-haiku-4-5-20251001', permissionMode: 'auto' } }
    createControlSession({ model: 'claude-haiku-4-5-20251001', agents }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    expect((capturedOptions as any).agents).toEqual(agents)
  })

  it('omits undefined fields (no effort/maxBudgetUsd/maxTurns/etc in options)', () => {
    createControlSession({ model: 'claude-haiku-4-5-20251001' }, registry)

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    const opts = capturedOptions as any
    expect(opts.effort).toBeUndefined()
    expect(opts.maxBudgetUsd).toBeUndefined()
    expect(opts.maxTurns).toBeUndefined()
    expect(opts.systemPrompt).toBeUndefined()
    expect(opts.outputFormat).toBeUndefined()
    expect(opts.plugins).toBeUndefined()
    expect(opts.hooks).toBeUndefined()
    expect(opts.agents).toBeUndefined()
  })

  it('snapshot of all forwarded fields', () => {
    createControlSession(
      {
        model: 'claude-haiku-4-5-20251001',
        effort: 'max',
        maxBudgetUsd: 10,
        maxTurns: 25,
        systemPrompt: 'Be helpful.',
        outputFormat: { type: 'json_schema', schema: { type: 'object' } },
        plugins: [{ type: 'local', path: '/p' }],
        hooks: { PostToolUse: [] },
        agents: { a: { model: 'claude-haiku-4-5-20251001' } },
      },
      registry,
    )

    expect(capturedOptions).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: testing SDK passthrough fields
    const opts = capturedOptions as any
    expect(opts).toMatchObject({
      effort: 'max',
      maxBudgetUsd: 10,
      maxTurns: 25,
      systemPrompt: 'Be helpful.',
      outputFormat: { type: 'json_schema', schema: { type: 'object' } },
      plugins: [{ type: 'local', path: '/p' }],
      hooks: { PostToolUse: [] },
      agents: { a: { model: 'claude-haiku-4-5-20251001' } },
    })
  })
})
