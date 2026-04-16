import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useLiveSessionStore } from '../../store/live-session-store'
import type { LiveSession } from '@claude-view/shared/types/generated'

/** Minimal LiveSession factory — only required fields. */
function makeSession(id: string): LiveSession {
  return {
    id,
    status: 'Working',
    startedAt: null,
    closedAt: null,
    control: null,
    model: null,
    contextWindowTokens: 0,
    agentState: { group: 'autonomous', state: 'acting', label: 'Working', context: null },
    pid: 1234,
    title: `Session ${id}`,
    lastUserMessage: '',
    currentActivity: 'Working',
    turnCount: 1,
    lastActivityAt: Date.now() / 1000,
    currentTurnStartedAt: null,
    subAgents: [],
    progressItems: [],
    compactCount: 0,
    hookEvents: [],
    project: 'test',
    projectDisplayName: 'test',
    projectPath: '/tmp/test',
    filePath: `/tmp/${id}.jsonl`,
    gitBranch: null,
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: null,
    tokens: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 },
    cost: {
      totalUsd: 0,
      inputUsd: 0,
      outputUsd: 0,
      cacheReadUsd: 0,
      cacheCreationUsd: 0,
      savingsUsd: 0,
    },
    cacheStatus: 'Unknown',
    lastTurnTaskSeconds: null,
    lastCacheHitAt: null,
    teamName: null,
    editCount: 0,
    toolsUsed: [],
    slug: null,
    phase: { labels: [], dominant: null, freshness: 'Stale' },
  } as unknown as LiveSession
}

describe('SSE event batcher', () => {
  beforeEach(() => {
    vi.stubGlobal('requestAnimationFrame', (cb: () => void) => setTimeout(cb, 0))
    vi.stubGlobal('cancelAnimationFrame', vi.fn())

    useLiveSessionStore.setState({
      sessionsById: new Map(),
      recentlyClosed: [],
      summary: null,
      connectionState: 'disconnected',
      isInitialized: false,
      lastUpdateTs: 0,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('store handles rapid upsert burst without data loss', () => {
    const store = useLiveSessionStore.getState()
    const sessions = Array.from({ length: 20 }, (_, i) => makeSession(`burst-${i}`))

    for (const s of sessions) {
      store.handleUpsert(s)
    }

    const state = useLiveSessionStore.getState()
    expect(state.sessionsById.size).toBe(20)
    for (let i = 0; i < 20; i++) {
      expect(state.sessionsById.has(`burst-${i}`)).toBe(true)
    }
  })

  it('store handles rapid remove burst preserving order', () => {
    const store = useLiveSessionStore.getState()
    const sessions = Array.from({ length: 20 }, (_, i) => makeSession(`rm-${i}`))

    for (const s of sessions) store.handleUpsert(s)
    expect(useLiveSessionStore.getState().sessionsById.size).toBe(20)

    for (const s of sessions) {
      useLiveSessionStore.getState().handleRemove(s.id, s)
    }

    const state = useLiveSessionStore.getState()
    expect(state.sessionsById.size).toBe(0)
    expect(state.recentlyClosed).toHaveLength(20)
    expect(state.recentlyClosed[0].id).toBe('rm-19')
    expect(state.recentlyClosed[19].id).toBe('rm-0')
  })

  it('rapid removes at capacity cap at 100', () => {
    const store = useLiveSessionStore.getState()
    const sessions = Array.from({ length: 110 }, (_, i) => makeSession(`cap-${i}`))

    for (const s of sessions) store.handleUpsert(s)
    for (const s of sessions) {
      useLiveSessionStore.getState().handleRemove(s.id, s)
    }

    const state = useLiveSessionStore.getState()
    expect(state.recentlyClosed.length).toBeLessThanOrEqual(100)
    expect(state.recentlyClosed[0].id).toBe('cap-109')
  })
})
