// use-conversation-loading-gate.test.ts
// Bug 2 TDD: Error banner on new sessions caused by history query firing before init completes.
// Tests the initComplete loading gate + suppressNotFound preservation + sessionState='initializing'.

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

// ── LG-01: initComplete=false gates history query (enabled=false) ────
// Bug: Without initComplete gate, history fires at mount with enabled=!isLive (true),
// gets 404 for new session (JSONL doesn't exist), shows red error banner.
// Fix: History uses enabled=source.initComplete, so query waits until init() resolves.
describe('LG-01: initComplete loading gate prevents premature history fetch', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('history query is disabled when initComplete=false (pre-init)', () => {
    // Simulate: session just mounted, init() not yet resolved
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'idle',
      isLive: false,
      initComplete: false,
    })

    renderHook(() => useConversation('new-session-1'), {
      wrapper: createWrapper(),
    })

    // useSessionMessages should be called with enabled=false
    // because initComplete is false — the query must NOT fire
    const lastCall = mockSessionMessages.mock.calls[mockSessionMessages.mock.calls.length - 1]
    expect(lastCall[1]).toHaveProperty('enabled', false)
  })

  it('history query is enabled when initComplete=true (post-init, not live)', () => {
    // Simulate: init() resolved, session is history-only (no active sidecar)
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'idle',
      isLive: false,
      initComplete: true,
    })

    renderHook(() => useConversation('history-session-1'), {
      wrapper: createWrapper(),
    })

    // useSessionMessages should be called with enabled=true
    // because initComplete is true and we have a sessionId
    const lastCall = mockSessionMessages.mock.calls[mockSessionMessages.mock.calls.length - 1]
    expect(lastCall[1]).toHaveProperty('enabled', true)
  })
})

// ── LG-02: suppressNotFound passed when session is initializing ──────
// The original code had suppressNotFound=isInitializing. The revert removed it.
// Fix restores it: when sessionState='initializing', suppressNotFound=true prevents
// 404 errors from showing the red banner for brand-new sessions.
describe('LG-02: suppressNotFound preserved when session is initializing', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockSessionSource.mockReturnValue({ ...defaultSource })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('passes suppressNotFound=true when sessionState is "initializing"', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'initializing',
      isLive: false,
      initComplete: true,
    })

    renderHook(() => useConversation('initializing-session'), {
      wrapper: createWrapper(),
    })

    const lastCall = mockSessionMessages.mock.calls[mockSessionMessages.mock.calls.length - 1]
    expect(lastCall[1]).toHaveProperty('suppressNotFound', true)
  })

  it('passes suppressNotFound=false when sessionState is NOT "initializing"', () => {
    mockSessionSource.mockReturnValue({
      ...defaultSource,
      sessionState: 'waiting_input',
      isLive: false,
      initComplete: true,
    })

    renderHook(() => useConversation('active-session'), {
      wrapper: createWrapper(),
    })

    const lastCall = mockSessionMessages.mock.calls[mockSessionMessages.mock.calls.length - 1]
    expect(lastCall[1]).toHaveProperty('suppressNotFound', false)
  })
})

// ── LG-03: skipWs bypasses initComplete gate ──
// Watching mode (skipWs=true) calls useSessionSource(undefined) which never runs init(),
// so initComplete stays false. History must still load via the skipWs bypass.
describe('LG-03: skipWs bypasses initComplete gate', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    // skipWs path: useSessionSource(undefined) → initComplete stays false
    mockSessionSource.mockReturnValue({ ...defaultSource, initComplete: false })
    mockSessionMessages.mockReturnValue({
      ...defaultMessages,
    } as unknown as ReturnType<typeof useSessionMessages>)
  })

  it('history query is enabled even when initComplete=false if skipWs=true', () => {
    renderHook(() => useConversation('test-session', { skipWs: true }), {
      wrapper: createWrapper(),
    })

    const lastCall = mockSessionMessages.mock.calls[mockSessionMessages.mock.calls.length - 1]
    expect(lastCall[1]).toHaveProperty('enabled', true)
  })
})
