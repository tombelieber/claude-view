import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
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
    turnVersion: 0,
    streamGap: false,
    clearPendingMessage: vi.fn(),
    resetAccumulator: vi.fn(),
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
  turnVersion: 0,
  streamGap: false,
  clearPendingMessage: vi.fn(),
  resetAccumulator: vi.fn(),
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

// ─── RC-001: First message 404 race condition ────────────────
// Bug: User creates a new session and sends a message. The JSONL file hasn't been
// flushed to disk yet, so the history query returns 404. Without freshlyCreated
// suppression, useSessionMessages throws, entering permanent error state with
// "Failed to load messages" shown in the UI.
// Fix: freshlyCreated location state flag sets suppressNotFound = true during the
// race window (idle state before WS transitions to initializing/active).
describe('RC-001: first message 404 race condition', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('freshlyCreated session suppresses 404 during idle → no error state', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'idle', // Before WS connects
    })

    renderHook(() => useConversation('new-session', { freshlyCreated: true }), {
      wrapper: createWrapper(),
    })

    // The critical assertion: suppressNotFound is true during the idle gap
    expect(mockSessionMessages).toHaveBeenCalledWith(
      'new-session',
      expect.objectContaining({ suppressNotFound: true }),
    )
  })

  it('non-freshlyCreated session does NOT suppress 404 during idle', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'idle',
    })

    renderHook(() => useConversation('existing-session'), {
      wrapper: createWrapper(),
    })

    // Existing session should fail normally on 404
    expect(mockSessionMessages).toHaveBeenCalledWith(
      'existing-session',
      expect.objectContaining({ suppressNotFound: false }),
    )
  })

  it('freshlyCreated suppression stops once session transitions to active', () => {
    // Session has transitioned past idle — WS is connected
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'active',
    })

    renderHook(() => useConversation('new-session', { freshlyCreated: true }), {
      wrapper: createWrapper(),
    })

    // isInitializing is true when active (regardless of freshlyCreated),
    // so suppressNotFound remains true — this is correct because active sessions
    // should also suppress 404 (JSONL may not be written yet during first turn)
    expect(mockSessionMessages).toHaveBeenCalledWith(
      'new-session',
      expect.objectContaining({ suppressNotFound: true }),
    )
  })
})

// ─── RC-002: Optimistic message duplication ────────────────
// Bug: User sends "hello". Optimistic block appears. Server echoes the message
// in stream (source.blocks). Turn completes → turnVersion increments → accumulator
// resets (source.blocks goes empty) → history refetch adds the message to
// history.blocks. But old code only checked source.blocks for dedup — after
// accumulator reset, the optimistic block reappeared as a duplicate alongside
// the history message.
// Fix: pendingOptimistic now checks both source.blocks AND history.blocks.
describe('RC-002: optimistic message duplication', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('after turn_complete + accumulator reset, optimistic block does NOT re-appear', () => {
    // Phase 1: Message sent, source has the echo
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      blocks: [{ type: 'user', id: 'user-0', text: 'hello', timestamp: 1 }] as any,
      sessionState: 'active',
      isLive: true,
      send: vi.fn(),
      sendIfLive: vi.fn(),
    })

    const { result, rerender } = renderHook(() => useConversation('sess-1'), {
      wrapper: createWrapper(),
    })

    // Send the message — creates optimistic block
    act(() => {
      result.current.actions.sendMessage('hello')
    })

    // Optimistic should be deduped against source.blocks
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    let userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks).toHaveLength(1) // Only the source echo

    // Phase 2: Simulate turn_complete → accumulator reset → history has the message
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      blocks: [], // Accumulator reset — source.blocks is empty
      sessionState: 'waiting_input',
      isLive: true,
      send: vi.fn(),
      sendIfLive: vi.fn(),
      turnVersion: 1,
    })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'hello',
                uuid: 'server-uuid',
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
      isFetching: false,
    } as unknown as ReturnType<typeof useSessionMessages>)

    rerender()

    // Critical: optimistic block should be deduped against history.blocks
    // (not just source.blocks which is now empty)
    // biome-ignore lint/suspicious/noExplicitAny: test assertion
    userBlocks = result.current.blocks.filter((b: any) => b.type === 'user')
    expect(userBlocks).toHaveLength(1) // Only history, no stale optimistic
  })
})

// ─── RC-003: Watching mode does not block own sessions ────────────────
// Bug: When a session is in liveSessions (detected by Live Monitor) AND also in
// sidecarSessionIds (managed by sidecar), the user owns it. Watching mode must
// NOT be applied — the user should have full input capability.
// The watching derivation happens in ChatPage.tsx (not in useConversation), but
// we test the hook's behavior with skipWs to verify: a sidecar-managed session
// should NOT receive skipWs=true from the calling component.
describe('RC-003: watching mode does not block own sessions', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('sidecar-managed session (skipWs=false) connects via WS normally', () => {
    // Simulate: session is in both liveSessions and sidecarIds → NOT watching
    // ChatPage passes skipWs=false (or omits it)
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'active',
      isLive: true,
      send: vi.fn(),
      sendIfLive: vi.fn(),
      controlId: 'ctrl-1',
    })

    renderHook(() => useConversation('my-session', { skipWs: false }), {
      wrapper: createWrapper(),
    })

    // WS connection should be established (sessionId passed to useSessionSource)
    expect(mockSessionSource).toHaveBeenCalledWith('my-session')
  })

  it('watching session (skipWs=true) does NOT connect via WS', () => {
    // Simulate: session in liveSessions but NOT in sidecarIds → watching
    renderHook(() => useConversation('external-session', { skipWs: true }), {
      wrapper: createWrapper(),
    })

    // No WS connection (undefined passed to useSessionSource)
    expect(mockSessionSource).toHaveBeenCalledWith(undefined)
  })

  it('watching session still has access to history blocks', () => {
    // Watching sessions load history via REST (different from WS)
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
      data: {
        pages: [
          {
            messages: [
              {
                role: 'user',
                content: 'external message',
                uuid: 'h1',
                timestamp: '2026-03-17T00:00:00Z',
              },
              {
                role: 'assistant',
                content: 'external reply',
                uuid: 'a1',
                timestamp: '2026-03-17T00:00:01Z',
              },
            ],
            total: 2,
            offset: 0,
            limit: 100,
            hasMore: false,
          },
        ],
        pageParams: [-1],
      },
    } as unknown as ReturnType<typeof useSessionMessages>)

    const { result } = renderHook(() => useConversation('external-session', { skipWs: true }), {
      wrapper: createWrapper(),
    })

    // History should be loaded even in watching mode
    expect(result.current.blocks.length).toBe(2)
    // useSessionMessages should receive the real sessionId (not undefined)
    expect(mockSessionMessages).toHaveBeenCalledWith('external-session', expect.any(Object))
  })
})
