import type { LiveSession } from '@claude-view/shared/types/generated'
import { beforeEach, describe, expect, it } from 'vitest'
import { type LiveSummary, getLastEventTime, useLiveSessionStore } from './live-session-store'

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

const emptySummary = { needsYouCount: 0, autonomousCount: 0 } as LiveSummary

function resetStore() {
  // handleSnapshot clears both Zustand state AND the module-level _eventTimes Map
  useLiveSessionStore.getState().handleSnapshot(emptySummary, [], [])
  // Reset to pre-initialized state
  useLiveSessionStore.setState({
    isInitialized: false,
    connectionState: 'disconnected',
    lastUpdateTs: 0,
  })
}

describe('useLiveSessionStore', () => {
  beforeEach(() => {
    resetStore()
  })

  // -----------------------------------------------------------------------
  // Regression: #53 — unstable references eliminated
  // -----------------------------------------------------------------------

  describe('no unstable references per event', () => {
    it('handleUpsert does NOT store lastEventTimes in Zustand state', () => {
      const state = useLiveSessionStore.getState()
      expect('lastEventTimes' in state).toBe(false)
    })

    it('handleUpsert does NOT store Date object in Zustand state', () => {
      const state = useLiveSessionStore.getState()
      expect('lastUpdate' in state).toBe(false)
      expect(typeof state.lastUpdateTs).toBe('number')
    })

    it('handleUpsert updates lastUpdateTs as a number', () => {
      const before = Date.now()
      useLiveSessionStore.getState().handleUpsert(makeSession('s1'))
      const after = Date.now()
      const ts = useLiveSessionStore.getState().lastUpdateTs
      expect(ts).toBeGreaterThanOrEqual(before)
      expect(ts).toBeLessThanOrEqual(after)
    })

    it('handleUpsert writes to module-level event times map', () => {
      expect(getLastEventTime('s1')).toBeUndefined()
      useLiveSessionStore.getState().handleUpsert(makeSession('s1'))
      expect(getLastEventTime('s1')).toBeGreaterThan(0)
    })
  })

  // -----------------------------------------------------------------------
  // Regression: #53 — handleRemove closed list cap
  // -----------------------------------------------------------------------

  describe('handleRemove — closed list is bounded', () => {
    it('prepends closed session and caps at 100', () => {
      const closedSessions = Array.from({ length: 100 }, (_, i) => makeSession(`closed-${i}`))
      useLiveSessionStore.getState().handleSnapshot(emptySummary, [], closedSessions)

      expect(useLiveSessionStore.getState().recentlyClosed).toHaveLength(100)

      const active = makeSession('active-1')
      useLiveSessionStore.getState().handleUpsert(active)
      useLiveSessionStore.getState().handleRemove('active-1', active)

      const closed = useLiveSessionStore.getState().recentlyClosed
      expect(closed).toHaveLength(100)
      expect(closed[0].id).toBe('active-1')
      expect(closed.find((s) => s.id === 'closed-99')).toBeUndefined()
    })

    it('does not exceed 100 even with rapid removals', () => {
      const sessions = Array.from({ length: 110 }, (_, i) => makeSession(`s-${i}`))
      const store = useLiveSessionStore.getState()

      for (const s of sessions) store.handleUpsert(s)
      for (const s of sessions) {
        useLiveSessionStore.getState().handleRemove(s.id, s)
      }

      expect(useLiveSessionStore.getState().recentlyClosed.length).toBeLessThanOrEqual(100)
    })

    it('cleans up event times on remove', () => {
      const s = makeSession('s1')
      useLiveSessionStore.getState().handleUpsert(s)
      expect(getLastEventTime('s1')).toBeDefined()

      useLiveSessionStore.getState().handleRemove('s1', s)
      expect(getLastEventTime('s1')).toBeUndefined()
    })
  })

  // -----------------------------------------------------------------------
  // handleSnapshot
  // -----------------------------------------------------------------------

  describe('handleSnapshot', () => {
    it('replaces all state and clears event times', () => {
      useLiveSessionStore.getState().handleUpsert(makeSession('old-1'))
      expect(getLastEventTime('old-1')).toBeDefined()

      const newSessions = [makeSession('new-1'), makeSession('new-2')]
      const newClosed = [makeSession('closed-1')]
      useLiveSessionStore
        .getState()
        .handleSnapshot(
          { needsYouCount: 1, autonomousCount: 1 } as LiveSummary,
          newSessions,
          newClosed,
        )

      const state = useLiveSessionStore.getState()
      expect(state.sessionsById.size).toBe(2)
      expect(state.recentlyClosed).toHaveLength(1)
      expect(state.isInitialized).toBe(true)
      expect(getLastEventTime('old-1')).toBeUndefined()
      expect(getLastEventTime('new-1')).toBeDefined()
      expect(getLastEventTime('new-2')).toBeDefined()
    })
  })

  // -----------------------------------------------------------------------
  // dismissSession / dismissAllClosed
  // -----------------------------------------------------------------------

  describe('dismiss', () => {
    it('dismissSession removes from closed list', async () => {
      const closed = [makeSession('c1'), makeSession('c2'), makeSession('c3')]
      useLiveSessionStore.getState().handleSnapshot(emptySummary, [], closed)

      await useLiveSessionStore.getState().dismissSession('c2')

      const remaining = useLiveSessionStore.getState().recentlyClosed
      expect(remaining).toHaveLength(2)
      expect(remaining.find((s) => s.id === 'c2')).toBeUndefined()
    })

    it('dismissAllClosed empties the list', async () => {
      const closed = [makeSession('c1'), makeSession('c2')]
      useLiveSessionStore.getState().handleSnapshot(emptySummary, [], closed)

      await useLiveSessionStore.getState().dismissAllClosed()

      expect(useLiveSessionStore.getState().recentlyClosed).toHaveLength(0)
    })
  })

  // -----------------------------------------------------------------------
  // Regression: duplicate cards in the Closed column.
  //
  // A session that is resurrected (re-activated after closing, e.g. a real
  // `--resume` or a late JSONL flush server-side) and then closes again
  // produced a SECOND closed card. Root cause: recentlyClosed was treated as
  // an append log, not a set. "Recently closed" is membership keyed by id:
  // a session is active XOR closed, and appears in the closed list AT MOST ONCE.
  // -----------------------------------------------------------------------

  describe('set invariant — closed column has no duplicates', () => {
    it('handleRemove is idempotent: a session appears in recentlyClosed at most once', () => {
      const s = makeSession('dup-1')
      const store = useLiveSessionStore.getState()
      store.handleRemove('dup-1', s) // close once
      store.handleRemove('dup-1', s) // resurrected then closed again → second remove

      const closed = useLiveSessionStore.getState().recentlyClosed
      expect(closed.filter((c) => c.id === 'dup-1')).toHaveLength(1)
    })

    it('re-close replaces the prior entry (most-recent wins, moved to front)', () => {
      const store = useLiveSessionStore.getState()
      store.handleRemove('a', makeSession('a'))
      store.handleRemove('b', makeSession('b'))
      // 'a' closes again carrying fresher data.
      const aNew = { ...makeSession('a'), title: 'a second life' } as LiveSession
      store.handleRemove('a', aNew)

      const closed = useLiveSessionStore.getState().recentlyClosed
      expect(closed.filter((c) => c.id === 'a')).toHaveLength(1)
      expect(closed[0].id).toBe('a') // newest close floats to the top
      expect(closed.find((c) => c.id === 'a')?.title).toBe('a second life')
    })

    it('handleUpsert pulls a resumed session back out of recentlyClosed (active XOR closed)', () => {
      const store = useLiveSessionStore.getState()
      const s = makeSession('resumed')
      store.handleRemove('resumed', s) // closed
      expect(useLiveSessionStore.getState().recentlyClosed.some((c) => c.id === 'resumed')).toBe(
        true,
      )

      store.handleUpsert(s) // resumed → active again
      const state = useLiveSessionStore.getState()
      expect(state.sessionsById.has('resumed')).toBe(true)
      expect(state.recentlyClosed.some((c) => c.id === 'resumed')).toBe(false)
    })

    it('handleSnapshot dedups recentlyClosed by id (truthful boundary normalization)', () => {
      const dup = makeSession('snap-dup')
      useLiveSessionStore
        .getState()
        .handleSnapshot(emptySummary, [], [dup, makeSession('other'), dup])

      const closed = useLiveSessionStore.getState().recentlyClosed
      expect(closed.filter((c) => c.id === 'snap-dup')).toHaveLength(1)
      expect(closed).toHaveLength(2) // snap-dup + other
    })

    it('handleSnapshot drops a closed id that is also active (active wins)', () => {
      const both = makeSession('both')
      useLiveSessionStore.getState().handleSnapshot(emptySummary, [both], [both])

      const state = useLiveSessionStore.getState()
      expect(state.sessionsById.has('both')).toBe(true)
      expect(state.recentlyClosed.some((c) => c.id === 'both')).toBe(false)
    })
  })
})
