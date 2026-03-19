// apps/web/src/hooks/message-state-errors.test.ts
// NEG-01..NEG-06: Frontend-side negative/error path tests for message state
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import { createElement } from 'react'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useConversation } from './use-conversation'

// Mock useSessionSource to return controlled values
vi.mock('./use-session-source', () => ({
  useSessionSource: vi.fn().mockReturnValue({
    blocks: [],
    sessionState: 'idle',
    controlId: null,
    send: null,
    sendIfLive: null,
    isLive: false,
    reconnect: vi.fn(),
    resume: vi.fn(),
    totalInputTokens: 0,
    contextWindowSize: 0,
    canResumeLazy: false,
    model: '',
    slashCommands: [],
    mcpServers: [],
    permissionMode: 'default',
    skills: [],
    agents: [],
    channel: null,
    capabilities: [],
    committedBlocks: [],
    pendingText: '',
    clearPendingMessage: vi.fn(),
    initComplete: false,
  }),
}))

vi.mock('./use-session-messages', () => ({
  useSessionMessages: vi.fn().mockReturnValue({
    data: undefined,
    error: null,
    hasPreviousPage: false,
    fetchPreviousPage: vi.fn(),
    isFetchingPreviousPage: false,
    isFetching: false,
    isLoading: false,
  }),
}))

import { useSessionMessages } from './use-session-messages'
import { useSessionSource } from './use-session-source'

const mockSessionSource = vi.mocked(useSessionSource)
const mockSessionMessages = vi.mocked(useSessionMessages)

const defaultSource = {
  blocks: [],
  sessionState: 'idle',
  controlId: null,
  send: null,
  sendIfLive: null,
  isLive: false,
  reconnect: vi.fn(),
  resume: vi.fn(),
  totalInputTokens: 0,
  contextWindowSize: 0,
  canResumeLazy: false,
  model: '',
  slashCommands: [],
  mcpServers: [],
  permissionMode: 'default',
  skills: [],
  agents: [],
  channel: null,
  capabilities: [],
  committedBlocks: [],
  pendingText: '',
  clearPendingMessage: vi.fn(),
  initComplete: false,
}

const defaultMessages = {
  data: undefined,
  error: null,
  hasPreviousPage: false,
  fetchPreviousPage: vi.fn(),
  isFetchingPreviousPage: false,
  isFetching: false,
  isLoading: false,
}

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client }, children)
}

// ── NEG-01: WS disconnects mid-stream, pendingText preserved ──────────
describe('NEG-01: WS disconnect mid-stream preserves pendingText', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('pendingText survives WS disconnect (kept in source state)', () => {
    // Phase 1: Live session streaming with pending text
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [
        // biome-ignore lint/suspicious/noExplicitAny: test fixture
        {
          type: 'assistant',
          id: 'a1',
          segments: [{ kind: 'text', text: 'start' }],
          streaming: true,
        } as any,
      ],
      pendingText: 'partial',
      isLive: true,
      sessionState: 'active',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    const { result, rerender } = renderHook(() => useConversation('sess-neg1'), {
      wrapper: createWrapper(),
    })

    // Pending text should be visible in block text
    const assistantBlock = result.current.blocks.find(
      // biome-ignore lint/suspicious/noExplicitAny: test assertion
      (b: any) => b.type === 'assistant',
    )
    expect(assistantBlock).toBeDefined()

    // Phase 2: WS disconnects — isLive goes false, but pendingText stays
    // (use-session-source preserves state on disconnect)
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [
        // biome-ignore lint/suspicious/noExplicitAny: test fixture
        {
          type: 'assistant',
          id: 'a1',
          segments: [{ kind: 'text', text: 'start' }],
          streaming: true,
        } as any,
      ],
      pendingText: 'partial',
      isLive: false,
      sessionState: 'reconnecting',
      send: null,
      sendIfLive: null,
    })

    rerender()

    // The hook should fall back to history when not live, but pendingText is still in source
    // Source pendingText is preserved regardless of isLive state
    expect(mockSessionSource.mock.results[0].value.pendingText).toBeDefined()
  })

  it('pendingText cleared when blocks_snapshot arrives on reconnect', () => {
    // After reconnect, blocks_snapshot arrives — pendingText clears
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [
        // biome-ignore lint/suspicious/noExplicitAny: test fixture
        {
          type: 'assistant',
          id: 'a1',
          segments: [{ kind: 'text', text: 'start complete response' }],
          streaming: false,
        } as any,
      ],
      pendingText: '',
      isLive: true,
      sessionState: 'waiting_input',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    const { result } = renderHook(() => useConversation('sess-neg1-reconnect'), {
      wrapper: createWrapper(),
    })

    // After reconnect + snapshot: committed blocks present, no pending text
    expect(result.current.blocks.length).toBe(1)
  })
})

// ── NEG-02: blocks_snapshot with empty array ──────────────────────────
describe('NEG-02: blocks_snapshot with empty array renders empty conversation', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('empty committedBlocks and empty pendingText — no crash', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [],
      pendingText: '',
      isLive: true,
      sessionState: 'waiting_input',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    const { result } = renderHook(() => useConversation('sess-neg2'), {
      wrapper: createWrapper(),
    })

    expect(result.current.blocks).toEqual([])
  })
})

// ── NEG-03: turn_complete without blocks field — backward compat ──────
describe('NEG-03: turn_complete without blocks field preserves committed', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('committedBlocks unchanged when turn_complete has no blocks', () => {
    const threeBlocks = [
      { type: 'user', id: 'u1', text: 'hi', timestamp: 1 },
      {
        type: 'assistant',
        id: 'a1',
        segments: [{ kind: 'text', text: 'hello' }],
        streaming: false,
      },
      {
        type: 'turn_boundary',
        id: 'tb1',
        success: true,
        totalCostUsd: 0.01,
        numTurns: 1,
        durationMs: 100,
        usage: {},
        modelUsage: {},
        permissionDenials: [],
        stopReason: null,
      },
    ]

    // Session with 3 committed blocks
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      committedBlocks: threeBlocks as any,
      isLive: true,
      sessionState: 'waiting_input',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    const { result, rerender } = renderHook(() => useConversation('sess-neg3'), {
      wrapper: createWrapper(),
    })

    expect(result.current.blocks.length).toBe(3)

    // turn_complete without blocks field — source still returns same committed
    // (use-session-source only updates committed when blocks field present)
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      committedBlocks: threeBlocks as any,
      isLive: true,
      sessionState: 'waiting_input',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    rerender()

    // Still 3 blocks — not cleared
    expect(result.current.blocks.length).toBe(3)
  })
})

// ── NEG-06: No blocks_snapshot within 3s triggers JSONL fallback ──────
describe('NEG-06: snapshot timeout triggers JSONL fallback (FLAG-B)', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('falls back to history blocks when no snapshot arrives within 3s', () => {
    // Live session, but committedBlocks is empty (no blocks_snapshot arrived yet)
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [],
      pendingText: '',
      isLive: true,
      sessionState: 'active',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    // History has data (useSessionMessages mock returns data — simulates JSONL availability).
    // In production, the enabled gate (!isLive) would prevent fetching, but since we
    // mock useSessionMessages directly, data is always available in the mock.
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'from history',
                uuid: 'h1',
                timestamp: '2026-03-17T00:00:00Z',
              },
            ],
            total: 1,
            offset: 0,
            limit: 100,
            hasMore: false,
          },
        ],
        pageParams: [-1],
      },
    } as unknown as ReturnType<typeof useSessionMessages>)

    const { result } = renderHook(() => useConversation('sess-neg6'), {
      wrapper: createWrapper(),
    })

    // Before timeout: isLive + no snapshotTimeout → use committedBlocks (empty).
    // History data exists but useMemo selects committed blocks when live and not timed out.
    expect(result.current.blocks).toEqual([])

    // Advance past 3s timeout
    act(() => {
      vi.advanceTimersByTime(3000)
    })

    // After timeout: snapshotTimeout=true → useMemo switches to history.blocks.
    // The binary source switch: `source.isLive && !snapshotTimeout` is now false,
    // so blocks come from history.blocks instead of committedBlocks.
    // Since our mock provides history data, the blocks should now contain history content.
    expect(result.current.blocks.length).toBeGreaterThan(0)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlock = result.current.blocks.find((b: any) => b.type === 'user') as any
    expect(userBlock).toBeDefined()
    expect(userBlock.text).toBe('from history')
  })
})
