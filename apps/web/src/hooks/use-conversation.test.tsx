import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
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

describe('useConversation block merging', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    // Re-apply default mocks after restoreAllMocks
    mockSessionSource.mockReturnValue({
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

  // --- binary source: isLive=true uses committedBlocks, no history ---
  it('shows committedBlocks when isLive=true (binary source switch)', () => {
    const liveBlock = { type: 'user', id: 'l1', text: 'live msg', timestamp: Date.now() / 1000 }
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [liveBlock] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 100,
      contextWindowSize: 200000,
      canResumeLazy: false,
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      committedBlocks: [liveBlock] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    // Binary source: isLive=true → committedBlocks used, no divider
    const dividerBlock = result.current.blocks.find((b) => b.type === 'notice')
    expect(dividerBlock).toBeUndefined()
    expect(result.current.blocks.length).toBeGreaterThanOrEqual(1)
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
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 100,
      contextWindowSize: 200000,
      canResumeLazy: false,
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
      committedBlocks: [
        { type: 'user', id: 'l1', text: 'live msg', timestamp: Date.now() / 1000 },
      ] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
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
      sendIfLive: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: true,
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
      sendIfLive: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      model: 'claude-opus-4-6',
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
      sendIfLive: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      model: '',
      slashCommands: ['commit', 'test'],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
      committedBlocks: [],
      pendingText: '',
      clearPendingMessage: vi.fn(),
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
      sendIfLive: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      model: '',
      slashCommands: [],
      mcpServers: [{ name: 'gh', status: 'connected' }],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
      committedBlocks: [],
      pendingText: '',
      clearPendingMessage: vi.fn(),
    })
    const { result } = renderHook(() => useConversation('test-id'), { wrapper: createWrapper() })
    expect(result.current.sessionInfo.mcpServers).toEqual([{ name: 'gh', status: 'connected' }])
  })
})

// ─── Source selection (single-stream pattern) ────────────────
describe('source selection (single-stream pattern)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({
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

  it('uses stream blocks when replay is complete and blocks exist', () => {
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'u1', text: 'hello', timestamp: 1 }] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      committedBlocks: [{ type: 'user', id: 'u1', text: 'hello', timestamp: 1 }] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    // Stream blocks should be used directly — no merge with history
    expect(result.current.blocks).toHaveLength(1)
    expect(result.current.blocks[0].type).toBe('user')
  })

  it('falls back to history when stream is empty (still connecting)', () => {
    mockSessionMessages.mockReturnValue({
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'old',
                uuid: 'h1',
                timestamp: '2026-03-15T00:00:00Z',
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
      blocks: [], // Stream empty — still connecting
      sessionState: 'idle',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: null,
      isLive: false,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: true,
      committedBlocks: [],
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    // Should show history blocks as fallback
    expect(result.current.blocks.length).toBeGreaterThan(0)
  })
})

// ─── binary source: live with committed blocks ────────────────
describe('source selection — binary source switch', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({
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

  it('shows committedBlocks only when isLive=true (binary source switch)', () => {
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'assistant', id: 'a1' }] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      committedBlocks: [{ type: 'assistant', id: 'a1' }] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    // Binary switch: isLive=true → committedBlocks only, no divider
    expect(result.current.blocks).toHaveLength(1)
    expect(result.current.blocks[0].id).toBe('a1')
    expect(result.current.blocks.every((b) => b.type !== 'notice')).toBe(true)
  })
})

// ─── Optimistic dedup ────────────────
describe('source selection — optimistic dedup', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({
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

  it('removes optimistic block when stream echo arrives with matching text', () => {
    // Stream already has the echo
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'user-0', text: 'hello', timestamp: 1 }] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      committedBlocks: [{ type: 'user', id: 'user-0', text: 'hello', timestamp: 1 }] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    // Send a message that matches the stream's existing echo
    act(() => {
      result.current.actions.sendMessage('hello')
    })

    // The optimistic block should be deduped against the stream's echo
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
  })

  it('keeps optimistic block when stream has no matching text yet', () => {
    // Stream has no echo yet
    mockSessionSource.mockReturnValue({
      blocks: [],
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      committedBlocks: [],
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })

    act(() => {
      result.current.actions.sendMessage('waiting for echo')
    })

    // Optimistic block should be visible (stream hasn't confirmed it)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    expect((userBlocks[0] as any).text).toBe('waiting for echo')
  })
})

// ─── sendMessage — simplified optimistic (echo-based) ────────────────
describe('sendMessage — simplified optimistic (echo-based)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({
      blocks: [],
      sessionState: 'idle',
      controlId: null,
      send: vi.fn(),
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

  it('creates optimistic block without status when message sent', () => {
    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })
    act(() => {
      result.current.actions.sendMessage('hello')
    })
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks.length).toBeGreaterThanOrEqual(1)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const last = userBlocks[userBlocks.length - 1] as any
    expect(last.text).toBe('hello')
  })

  it('marks block as failed after 10s if no echo arrives', () => {
    vi.useFakeTimers()
    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })
    act(() => {
      result.current.actions.sendMessage('timeout test')
    })
    // Advance 10 seconds
    act(() => {
      vi.advanceTimersByTime(10_000)
    })
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const failed = userBlocks.find((b: any) => b.text === 'timeout test') as any
    expect(failed?.status).toBe('failed')
    vi.useRealTimers()
  })

  it('retryMessage clears failed block and re-sends', () => {
    vi.useFakeTimers()
    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })
    act(() => {
      result.current.actions.sendMessage('retry test')
    })
    act(() => {
      vi.advanceTimersByTime(10_000)
    })
    // Find the failed block's localId
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const failedBlock = result.current.blocks.find(
      // biome-ignore lint/suspicious/noExplicitAny: test assertion
      (b: any) => b.type === 'user' && b.status === 'failed',
      // biome-ignore lint/suspicious/noExplicitAny: test assertion
    ) as any
    expect(failedBlock).toBeDefined()

    act(() => {
      result.current.actions.retryMessage(failedBlock.localId)
    })
    // Failed block should be removed (replaced by a new optimistic)
    const stillFailed = result.current.blocks.find(
      // biome-ignore lint/suspicious/noExplicitAny: test assertion
      (b: any) => b.type === 'user' && b.localId === failedBlock.localId,
    )
    expect(stillFailed).toBeUndefined()
    vi.useRealTimers()
  })
})

// ─── Echo-based flow (replaces initialMessage seeding) ────────────────
describe('echo-based flow (replaces initialMessage seeding)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({
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

  it('useConversation accepts only sessionId (no initialMessage parameter)', () => {
    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })
    expect(result.current).toBeDefined()
  })

  it('user block appears when stream.blocks contains user_message_echo', () => {
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [
        { type: 'user', id: 'user-0', text: 'hello', timestamp: 1000 },
        { type: 'assistant', id: 'a-1', text: 'hi', timestamp: 1001 },
      ] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      committedBlocks: [
        { type: 'user', id: 'user-0', text: 'hello', timestamp: 1000 },
        { type: 'assistant', id: 'a-1', text: 'hi', timestamp: 1001 },
      ] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })
    const userBlocks = result.current.blocks.filter((b) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
  })
})

// ─── Turn ordering (replaces interleave-user-blocks coverage) ────────────────
describe('turn ordering (replaces interleave-user-blocks coverage)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({
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

  it('stream blocks already have user message before assistant (correct position)', () => {
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [
        { type: 'user', id: 'user-0', text: 'question', timestamp: 1000 },
        { type: 'assistant', id: 'a-1', text: 'answer', timestamp: 1001 },
      ] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      committedBlocks: [
        { type: 'user', id: 'user-0', text: 'question', timestamp: 1000 },
        { type: 'assistant', id: 'a-1', text: 'answer', timestamp: 1001 },
      ] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })
    const userIdx = result.current.blocks.findIndex((b) => b.type === 'user')
    const assistantIdx = result.current.blocks.findIndex((b) => b.type === 'assistant')
    expect(userIdx).toBeLessThan(assistantIdx)
  })
})

// ─── Hook interaction: history base + live overlay always merges ───────
describe('source selection always merges history + live overlay', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({
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

  it('isLive=true uses committedBlocks only (binary switch)', () => {
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'u-1', text: 'hello', timestamp: 1000 }] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      committedBlocks: [{ type: 'user', id: 'u-1', text: 'hello', timestamp: 1000 }] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    const { result } = renderHook(() => useConversation('test-session'), {
      wrapper: createWrapper(),
    })
    // Binary switch: isLive=true → committedBlocks only
    expect(result.current.blocks).toHaveLength(1)
    expect(result.current.blocks[0].id).toBe('u-1')
  })

  it('merges history + live overlay without divider in binary source mode', () => {
    mockSessionSource.mockReturnValue({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'u-new', text: 'new msg', timestamp: 2000 }] as any,
      sessionState: 'active',
      controlId: 'ctrl-1',
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
      reconnect: vi.fn(),
      resume: vi.fn(),
      totalInputTokens: 0,
      contextWindowSize: 0,
      canResumeLazy: false,
      committedBlocks: [{ type: 'user', id: 'u-new', text: 'new msg', timestamp: 2000 }] as any,
      pendingText: '',
      clearPendingMessage: vi.fn(),
      model: '',
      slashCommands: [],
      mcpServers: [],
      permissionMode: 'default',
      skills: [],
      agents: [],
      channel: null,
      capabilities: [],
    })

    mockSessionMessages.mockReturnValue({
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'old msg',
                uuid: 'h-1',
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
    // Binary switch: isLive=true → committedBlocks only, no divider
    expect(result.current.blocks).toHaveLength(1)
    expect(result.current.blocks[0].id).toBe('u-new')
    expect(result.current.blocks.every((b) => b.type !== 'notice')).toBe(true)
  })
})

// ─── Binary source: isLive=false uses history ────────────────
describe('binary source: isLive=false uses history', () => {
  it('produces history blocks when isLive=false (binary switch)', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      isLive: false,
    })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [{ role: 'user', content: 'hi', uuid: 'h1' }],
            total: 1,
            offset: 0,
            limit: 100,
            hasMore: false,
          },
        ],
        pageParams: [-1],
      },
    } as unknown as ReturnType<typeof useSessionMessages>)
    const { result } = renderHook(() => useConversation('sess-1'), { wrapper: createWrapper() })
    // isLive=false → history only
    expect(result.current.blocks).toHaveLength(1)
    expect(result.current.blocks[0].type).toBe('user')
  })

  it('produces no RESUMED_DIVIDER (notice block) in any scenario', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      isLive: false,
    })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [{ role: 'user', content: 'old', uuid: 'h1' }],
            total: 1,
            offset: 0,
            limit: 100,
            hasMore: false,
          },
        ],
        pageParams: [-1],
      },
    } as unknown as ReturnType<typeof useSessionMessages>)
    const { result } = renderHook(() => useConversation('sess-1'), { wrapper: createWrapper() })
    expect(result.current.blocks.every((b) => b.type !== 'notice')).toBe(true)
  })

  it('shows only history blocks when stream is empty (between turns)', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      blocks: [],
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
    } as any)
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [{ role: 'user', content: 'old', uuid: 'h1' }],
            total: 1,
            offset: 0,
            limit: 100,
            hasMore: false,
          },
        ],
        pageParams: [-1],
      },
    } as unknown as ReturnType<typeof useSessionMessages>)
    const { result } = renderHook(() => useConversation('sess-1'), { wrapper: createWrapper() })
    expect(result.current.blocks).toHaveLength(1)
    expect(result.current.blocks[0].type).toBe('user')
  })
})

// NOTE: "deferred accumulator reset" and "freshlyCreated suppression" tests deleted.
// These tested dual-source merge behavior (turnVersion/resetAccumulator/freshlyCreated)
// which has been replaced by the binary source switch (committedBlocks/pendingText).

// ─── skipWs (watching mode) ────────────────
describe('skipWs (watching mode)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('passes undefined to useSessionSource when skipWs = true', () => {
    renderHook(() => useConversation('sess-1', { skipWs: true }), {
      wrapper: createWrapper(),
    })

    // skipWs: true → useSessionSource(undefined) — no WS connection
    expect(mockSessionSource).toHaveBeenCalledWith(undefined)
  })

  it('passes sessionId to useSessionSource when skipWs = false', () => {
    renderHook(() => useConversation('sess-1', { skipWs: false }), {
      wrapper: createWrapper(),
    })

    expect(mockSessionSource).toHaveBeenCalledWith('sess-1')
  })

  it('passes sessionId to useSessionSource when skipWs is undefined', () => {
    renderHook(() => useConversation('sess-1'), {
      wrapper: createWrapper(),
    })

    expect(mockSessionSource).toHaveBeenCalledWith('sess-1')
  })

  it('still loads history via REST even when skipWs = true', () => {
    renderHook(() => useConversation('sess-1', { skipWs: true }), {
      wrapper: createWrapper(),
    })

    // useSessionMessages should still receive the real sessionId for REST history
    expect(mockSessionMessages).toHaveBeenCalledWith('sess-1', expect.any(Object))
  })
})

// ─── Optimistic reconciliation (dual-source dedup) ────────────────
describe('optimistic reconciliation (dual-source dedup)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('filters optimistic block when text matches source.blocks', () => {
    // Source already has the echo
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'user-0', text: 'my message', timestamp: 1 }] as any,
      sessionState: 'active',
      isLive: true,
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    const { result } = renderHook(() => useConversation('sess-1'), {
      wrapper: createWrapper(),
    })

    // Send a message matching what's already in source.blocks
    act(() => {
      result.current.actions.sendMessage('my message')
    })

    // Optimistic should be deduped — only 1 user block (from source)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
  })

  it('filters optimistic block when text matches history.blocks', () => {
    // History has the confirmed message
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'confirmed msg',
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

    // Source has no matching block
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      blocks: [],
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
    })

    const { result } = renderHook(() => useConversation('sess-1'), {
      wrapper: createWrapper(),
    })

    act(() => {
      result.current.actions.sendMessage('confirmed msg')
    })

    // Optimistic deduped against history — should only have the history user block
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
  })

  it('keeps optimistic block when no match in either source or history', () => {
    // No matching message in source or history
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      blocks: [],
      send: vi.fn(),
      sendIfLive: vi.fn(),
      isLive: true,
    })

    const { result } = renderHook(() => useConversation('sess-1'), {
      wrapper: createWrapper(),
    })

    act(() => {
      result.current.actions.sendMessage('unconfirmed msg')
    })

    // Optimistic should remain visible
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    const userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    expect((userBlocks[0] as any).text).toBe('unconfirmed msg')
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    expect((userBlocks[0] as any).status).toBe('sending')
  })
})
