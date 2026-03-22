// sidecar/src/build-query-options.test.ts
// Tests for buildQueryOptions (private function) — verified indirectly via
// createControlSession and forkControlSession which pass options to SDK query().

import type { Options } from '@anthropic-ai/claude-agent-sdk'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SessionRegistry } from './session-registry.js'

// Capture the Options passed to the SDK query() function
let capturedOptions: Options | undefined

const mockGetSessionInfo = vi.fn()

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
  getSessionInfo: (...args: unknown[]) => mockGetSessionInfo(...args),
}))

vi.mock('./cli-path.js', () => ({
  findClaudeExecutable: vi.fn().mockReturnValue('/usr/local/bin/claude'),
}))

// Import AFTER mocks
const { createControlSession, forkControlSession, resumeControlSession } = await import(
  './sdk-session.js'
)

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

describe('resumeControlSession — projectPath fallback from getSessionInfo.cwd', () => {
  let registry: SessionRegistry

  beforeEach(() => {
    registry = new SessionRegistry()
    capturedOptions = undefined
    mockGetSessionInfo.mockReset()
  })

  it('uses info.cwd when req.projectPath is missing (inactive session resume)', async () => {
    mockGetSessionInfo.mockResolvedValue({
      sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      summary: 'test session',
      lastModified: Date.now(),
      cwd: '/Users/test/original-project',
    })

    await resumeControlSession({ sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee' }, registry)

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.cwd).toBe('/Users/test/original-project')
  })

  it('prefers req.projectPath over info.cwd when both are available', async () => {
    mockGetSessionInfo.mockResolvedValue({
      sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      summary: 'test session',
      lastModified: Date.now(),
      cwd: '/Users/test/original-project',
    })

    await resumeControlSession(
      {
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
        projectPath: '/Users/test/explicit-path',
      },
      registry,
    )

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.cwd).toBe('/Users/test/explicit-path')
  })

  it('falls back to process.cwd() when both req.projectPath and info.cwd are missing', async () => {
    mockGetSessionInfo.mockResolvedValue({
      sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      summary: 'test session',
      lastModified: Date.now(),
      // no cwd field
    })

    await resumeControlSession({ sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee' }, registry)

    expect(capturedOptions).toBeDefined()
    expect(capturedOptions!.cwd).toBe(process.cwd())
  })
})
