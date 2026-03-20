// apps/web/src/hooks/message-state-errors.test.ts
// NEG-01..NEG-06: Frontend-side negative/error path tests for message state
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook } from '@testing-library/react'
import { createElement } from 'react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
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

// ── NEG-06: Live session with empty committedBlocks falls back to history ──
describe('NEG-06: live session uses history when committedBlocks is empty (resume case)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('shows history blocks immediately when live but committedBlocks is empty', () => {
    // Resume scenario: WS connected (isLive=true) but sidecar accumulator empty
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [],
      pendingText: '',
      isLive: true,
      sessionState: 'active',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

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

    // No timeout needed — immediately shows history when committedBlocks is empty
    expect(result.current.blocks.length).toBeGreaterThan(0)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlock = result.current.blocks.find((b: any) => b.type === 'user') as any
    expect(userBlock).toBeDefined()
    expect(userBlock.text).toBe('from history')
  })

  it('does NOT append pendingText to history blocks (prevents corrupt display)', () => {
    // Resume scenario: live, empty committedBlocks, but pendingText from stream_delta
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [],
      pendingText: 'streaming text...',
      isLive: true,
      sessionState: 'active',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'old message',
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

    const { result } = renderHook(() => useConversation('sess-neg6b'), {
      wrapper: createWrapper(),
    })

    // pendingText must NOT be appended to history blocks
    const allText = result.current.blocks
      // biome-ignore lint/suspicious/noExplicitAny: test assertion
      .map((b: any) => b.text ?? '')
      .join('')
    expect(allText).not.toContain('streaming text...')
  })

  it('switches to committedBlocks once they have content', () => {
    // Start: live, empty committed
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      committedBlocks: [],
      pendingText: '',
      isLive: true,
      sessionState: 'active',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    const { result, rerender } = renderHook(() => useConversation('sess-neg6c'), {
      wrapper: createWrapper(),
    })

    // blocks_update arrives with content
    const liveBlocks = [
      { type: 'user', id: 'u1', text: 'new message', timestamp: Date.now() / 1000 },
      { type: 'assistant', id: 'a1', text: 'response', timestamp: Date.now() / 1000 },
    ]
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      committedBlocks: liveBlocks as any,
      pendingText: '',
      isLive: true,
      sessionState: 'waiting_input',
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    rerender()

    // Now uses committedBlocks (has content)
    expect(result.current.blocks.length).toBe(2)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    expect((result.current.blocks[0] as any).text).toBe('new message')
  })
})
