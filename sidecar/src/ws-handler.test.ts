import { beforeEach, describe, expect, it, vi } from 'vitest'
import { handleWebSocket } from './ws-handler.js'

function createMockWs() {
  const listeners: Record<string, (...args: unknown[]) => void> = {}
  return {
    send: vi.fn(),
    close: vi.fn(),
    readyState: 1,
    OPEN: 1,
    on: vi.fn((event: string, cb: (...args: unknown[]) => void) => {
      listeners[event] = cb
    }),
    _listeners: listeners,
  }
}

describe('ws-handler requestId echo', () => {
  // biome-ignore lint/suspicious/noExplicitAny: test mocks
  let mockWs: any
  let messageHandler: (raw: Buffer) => Promise<void>
  // biome-ignore lint/suspicious/noExplicitAny: test mocks
  let mockSession: any

  beforeEach(() => {
    const listeners: Record<string, (...args: unknown[]) => void> = {}
    mockWs = {
      send: vi.fn(),
      close: vi.fn(),
      readyState: 1,
      OPEN: 1,
      on: vi.fn((event: string, cb: (...args: unknown[]) => void) => {
        listeners[event] = cb
      }),
    }

    mockSession = {
      activeWs: null,
      state: 'active',
      emitter: { on: vi.fn(), removeListener: vi.fn() },
      eventBuffer: { getAfter: vi.fn().mockReturnValue([]) },
      permissions: {
        resolvePermission: vi.fn(),
        resolveQuestion: vi.fn(),
        resolvePlan: vi.fn(),
        resolveElicitation: vi.fn(),
        drainInteractive: vi.fn(),
      },
      query: {
        supportedModels: vi.fn().mockResolvedValue([{ id: 'claude-sonnet-4-20250514' }]),
        supportedCommands: vi.fn().mockResolvedValue([{ name: '/help' }]),
        supportedAgents: vi.fn().mockResolvedValue([]),
        mcpServerStatus: vi.fn().mockResolvedValue([{ name: 'gh', status: 'connected' }]),
        accountInfo: vi.fn().mockResolvedValue({ plan: 'pro' }),
        setMcpServers: vi.fn().mockResolvedValue({ ok: true }),
        rewindFiles: vi.fn().mockResolvedValue({ files: ['a.ts'] }),
      },
    }

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    const mockRegistry: any = {
      get: vi.fn().mockReturnValue(mockSession),
      emitSequenced: vi.fn(),
    }

    handleWebSocket(mockWs, 'ctrl-1', mockRegistry)
    messageHandler = listeners['message'] as (raw: Buffer) => Promise<void>
  })

  async function sendMsg(msg: Record<string, unknown>) {
    await messageHandler(Buffer.from(JSON.stringify(msg)))
  }

  function lastSentJson(): Record<string, unknown> {
    const calls = mockWs.send.mock.calls
    const lastCall = calls[calls.length - 1][0]
    return JSON.parse(lastCall)
  }

  it('query_models echoes requestId', async () => {
    await sendMsg({ type: 'query_models', requestId: 'req-1' })
    const resp = lastSentJson()
    expect(resp.type).toBe('query_result')
    expect(resp.queryType).toBe('models')
    expect(resp.requestId).toBe('req-1')
  })

  it('query_commands echoes requestId', async () => {
    await sendMsg({ type: 'query_commands', requestId: 'req-2' })
    expect(lastSentJson().requestId).toBe('req-2')
  })

  it('query_agents echoes requestId', async () => {
    await sendMsg({ type: 'query_agents', requestId: 'req-3' })
    expect(lastSentJson().requestId).toBe('req-3')
  })

  it('query_mcp_status echoes requestId', async () => {
    await sendMsg({ type: 'query_mcp_status', requestId: 'req-4' })
    expect(lastSentJson().requestId).toBe('req-4')
  })

  it('query_account_info echoes requestId', async () => {
    await sendMsg({ type: 'query_account_info', requestId: 'req-5' })
    expect(lastSentJson().requestId).toBe('req-5')
  })

  it('set_mcp_servers echoes requestId', async () => {
    await sendMsg({ type: 'set_mcp_servers', servers: {}, requestId: 'req-6' })
    const resp = lastSentJson()
    expect(resp.type).toBe('mcp_set_result')
    expect(resp.requestId).toBe('req-6')
  })

  it('rewind_files echoes requestId', async () => {
    await sendMsg({ type: 'rewind_files', userMessageId: 'u1', requestId: 'req-7' })
    const resp = lastSentJson()
    expect(resp.type).toBe('rewind_result')
    expect(resp.requestId).toBe('req-7')
  })

  it('response works without requestId (backwards compat)', async () => {
    await sendMsg({ type: 'query_models' })
    const resp = lastSentJson()
    expect(resp.type).toBe('query_result')
    expect(resp.requestId).toBeUndefined()
  })

  describe('One WS per session', () => {
    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    let wsRegistry: any
    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    let wsSession: any

    beforeEach(() => {
      wsSession = {
        activeWs: null,
        state: 'active',
        emitter: { on: vi.fn(), removeListener: vi.fn() },
        eventBuffer: { getAfter: vi.fn().mockReturnValue([]) },
        permissions: {
          resolvePermission: vi.fn(),
          resolveQuestion: vi.fn(),
          resolvePlan: vi.fn(),
          resolveElicitation: vi.fn(),
          drainInteractive: vi.fn(),
        },
        query: {
          supportedModels: vi.fn().mockResolvedValue([]),
          supportedCommands: vi.fn().mockResolvedValue([]),
          supportedAgents: vi.fn().mockResolvedValue([]),
          mcpServerStatus: vi.fn().mockResolvedValue([]),
          accountInfo: vi.fn().mockResolvedValue({}),
          setMcpServers: vi.fn().mockResolvedValue({ ok: true }),
          rewindFiles: vi.fn().mockResolvedValue({ files: [] }),
        },
      }

      wsRegistry = {
        get: vi.fn().mockReturnValue(wsSession),
        emitSequenced: vi.fn(),
      }
    })

    it('closes old WS with code 4001 when new WS connects to same session', () => {
      const oldWs = createMockWs()
      const newWs = createMockWs()

      handleWebSocket(oldWs as never, 'ctrl-1', wsRegistry)
      expect(wsSession.activeWs).toBe(oldWs)

      handleWebSocket(newWs as never, 'ctrl-1', wsRegistry)
      expect(oldWs.close).toHaveBeenCalledWith(4001, 'replaced_by_new_connection')
      expect(wsSession.activeWs).toBe(newWs)
    })

    it('does not close when session has no previous WS', () => {
      const ws = createMockWs()
      handleWebSocket(ws as never, 'ctrl-1', wsRegistry)
      expect(ws.close).not.toHaveBeenCalled()
      expect(wsSession.activeWs).toBe(ws)
    })

    it('clears activeWs on close only if it matches current WS', () => {
      const ws = createMockWs()
      handleWebSocket(ws as never, 'ctrl-1', wsRegistry)
      expect(wsSession.activeWs).toBe(ws)

      // Trigger close handler
      const closeHandler = ws._listeners['close']
      closeHandler()

      expect(wsSession.activeWs).toBeNull()
    })

    it('does NOT clear activeWs if a newer WS replaced it before close fires', () => {
      const oldWs = createMockWs()
      const newWs = createMockWs()

      handleWebSocket(oldWs as never, 'ctrl-1', wsRegistry)
      handleWebSocket(newWs as never, 'ctrl-1', wsRegistry)

      // Old WS close fires late — should NOT clear activeWs (newWs owns it now)
      const oldCloseHandler = oldWs._listeners['close']
      oldCloseHandler()

      expect(wsSession.activeWs).toBe(newWs)
    })
  })
})
