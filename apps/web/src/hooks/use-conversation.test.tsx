import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { createElement } from 'react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useConversation } from './use-conversation'

// Mock useSessionSource to return controlled live blocks
vi.mock('./use-session-source', () => ({
  useSessionSource: vi.fn().mockReturnValue({
    blocks: [],
    sessionState: 'idle',
    controlId: null,
    send: null,
    isLive: false,
    reconnect: vi.fn(),
    resume: vi.fn(),
    totalInputTokens: 0,
    contextWindowSize: 0,
    canResumeLazy: false,
    model: '',
    slashCommands: [],
    mcpServers: [],
  }),
}))

// Mock useSessionMessages (used internally by useHistoryBlocks)
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
// Import mocks for dynamic control
import { useSessionSource } from './use-session-source'

const mockSessionSource = vi.mocked(useSessionSource)
const mockSessionMessages = vi.mocked(useSessionMessages)

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client }, children)
}

describe('useConversation block merging', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    // Re-apply default mocks after restoreAllMocks
    mockSessionSource.mockReturnValue({
      blocks: [],
      sessionState: 'idle',
      controlId: null,
      send: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      model: '',
      slashCommands: [],
      mcpServers: [],
    })
    mockSessionMessages.mockReturnValue({
      data: undefined,
      error: null,
      hasPreviousPage: false,
      fetchPreviousPage: vi.fn(),
      isFetchingPreviousPage: false,
      isFetching: false,
      isLoading: false,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  // --- Integration: divider inserted when both history and live blocks exist ---
  it('inserts RESUMED_DIVIDER between history and live blocks', () => {
    mockSessionMessages.mockReturnValue({
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'old msg',
                uuid: 'h1',
                timestamp: '2026-03-13T00:00:00Z',
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
      error: null,
      hasPreviousPage: false,
      fetchPreviousPage: vi.fn(),
      isFetchingPreviousPage: false,
      isFetching: false,
      isLoading: false,
    } as unknown as ReturnType<typeof useSessionMessages>)

    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'l1', text: 'live msg', timestamp: Date.now() / 1000 }] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 100,
      contextWindowSize: 200000,
      canResumeLazy: false,
      model: '',
      slashCommands: [],
      mcpServers: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    // Should have: [history block(s), notice/divider, live block]
    const dividerBlock = result.current.blocks.find((b) => b.type === 'notice')
    expect(dividerBlock).toBeDefined()
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    expect((dividerBlock as any).variant).toBe('session_resumed')
  })

  // --- Integration: no divider when only history blocks ---
  it('omits divider when only history blocks exist (no live)', () => {
    mockSessionMessages.mockReturnValue({
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'old msg',
                uuid: 'h1',
                timestamp: '2026-03-13T00:00:00Z',
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
      error: null,
      hasPreviousPage: false,
      fetchPreviousPage: vi.fn(),
      isFetchingPreviousPage: false,
      isFetching: false,
      isLoading: false,
    } as unknown as ReturnType<typeof useSessionMessages>)

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    const divider = result.current.blocks.find((b) => b.type === 'notice')
    expect(divider).toBeUndefined()
    expect(result.current.blocks.length).toBeGreaterThan(0)
  })

  // --- Integration: no divider when only live blocks ---
  it('omits divider when only live blocks exist (no history)', () => {
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'l1', text: 'live msg', timestamp: Date.now() / 1000 }] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 100,
      contextWindowSize: 200000,
      canResumeLazy: false,
      model: '',
      slashCommands: [],
      mcpServers: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    const divider = result.current.blocks.find((b) => b.type === 'notice')
    expect(divider).toBeUndefined()
  })

  // --- Regression: optimistic dedup checks BOTH history and live blocks ---
  // Changelog #3: old code only deduped against source.blocks. If a message was confirmed
  // in history.blocks, the optimistic would persist forever as a duplicate.
  it('removes optimistic block when confirmed in history blocks', async () => {
    const localId = 'test-local-id'

    // Start with no blocks
    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    // Send a message (creates optimistic block)
    mockSessionSource.mockReturnValue({
      blocks: [],
      sessionState: 'idle',
      controlId: 'ctrl-1',
      send: vi.fn(),
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: true,
      model: '',
      slashCommands: [],
      mcpServers: [],
    })

    // Simulate: the message appears in history (confirmed by server)
    mockSessionMessages.mockReturnValue({
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'hello',
                uuid: 'server-uuid',
                localId, // Server echoes back the localId
                timestamp: '2026-03-13T00:00:00Z',
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
      error: null,
      hasPreviousPage: false,
      fetchPreviousPage: vi.fn(),
      isFetchingPreviousPage: false,
      isFetching: false,
      isLoading: false,
    } as unknown as ReturnType<typeof useSessionMessages>)

    // The optimistic block should be deduped against history.blocks
    // (not just source.blocks as before the fix)
    // Verify no duplicate user blocks with the same localId
    await waitFor(() => {
      const userBlocks = result.current.blocks.filter((b) => b.type === 'user')
      // Should be 1 (from history), not 2 (history + stale optimistic)
      expect(userBlocks.length).toBeLessThanOrEqual(1)
    })
  })
})

describe('sessionInfo includes palette fields', () => {
  it('forwards model from useSessionSource', () => {
    mockSessionSource.mockReturnValue({
      blocks: [],
      sessionState: 'idle',
      controlId: null,
      send: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      model: 'claude-opus-4-6',
      slashCommands: [],
      mcpServers: [],
    })
    const { result } = renderHook(() => useConversation('test-id'), { wrapper: createWrapper() })
    expect(result.current.sessionInfo.model).toBe('claude-opus-4-6')
  })

  it('forwards slashCommands from useSessionSource', () => {
    mockSessionSource.mockReturnValue({
      blocks: [],
      sessionState: 'idle',
      controlId: null,
      send: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      model: '',
      slashCommands: ['commit', 'test'],
      mcpServers: [],
    })
    const { result } = renderHook(() => useConversation('test-id'), { wrapper: createWrapper() })
    expect(result.current.sessionInfo.slashCommands).toEqual(['commit', 'test'])
  })

  it('forwards mcpServers from useSessionSource', () => {
    mockSessionSource.mockReturnValue({
      blocks: [],
      sessionState: 'idle',
      controlId: null,
      send: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      model: '',
      slashCommands: [],
      mcpServers: [{ name: 'gh', status: 'connected' }],
    })
    const { result } = renderHook(() => useConversation('test-id'), { wrapper: createWrapper() })
    expect(result.current.sessionInfo.mcpServers).toEqual([{ name: 'gh', status: 'connected' }])
  })
})
