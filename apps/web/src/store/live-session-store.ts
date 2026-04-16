import { useMemo } from 'react'
import { create } from 'zustand'
import type { LiveSession } from '@claude-view/shared/types/generated'

export interface LiveSummary {
  needsYouCount: number
  autonomousCount: number
  totalCostTodayUsd: number
  totalTokensToday: number
  processCount: number
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  inputCostUsd: number
  outputCostUsd: number
  cacheReadCostUsd: number
  cacheCreationCostUsd: number
  cacheSavingsUsd: number
}

type ConnectionState = 'connected' | 'reconnecting' | 'disconnected'

// ---------------------------------------------------------------------------
// Mutable event-time tracker — lives OUTSIDE Zustand to avoid per-event
// Map copies that create unstable references and cascade re-renders.
// Consumers read via `getLastEventTime()` or `useStalledSessions()`.
// ---------------------------------------------------------------------------
const _eventTimes = new Map<string, number>()

export function getLastEventTime(sessionId: string): number | undefined {
  return _eventTimes.get(sessionId)
}

/** Read the raw mutable map — only for derivation hooks, not for React state. */
export function getEventTimesSnapshot(): ReadonlyMap<string, number> {
  return _eventTimes
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

interface LiveSessionState {
  sessionsById: Map<string, LiveSession>
  recentlyClosed: LiveSession[]
  summary: LiveSummary | null
  connectionState: ConnectionState
  isInitialized: boolean
  /** Epoch-ms of the last SSE event (number, not Date — no object allocation). */
  lastUpdateTs: number

  handleSnapshot: (
    summary: LiveSummary,
    sessions: LiveSession[],
    recentlyClosed: LiveSession[],
  ) => void
  handleUpsert: (session: LiveSession) => void
  handleRemove: (sessionId: string, session: LiveSession) => void
  setConnectionState: (state: ConnectionState) => void
  dismissSession: (sessionId: string) => Promise<void>
  dismissAllClosed: () => Promise<void>
}

export const useLiveSessionStore = create<LiveSessionState>((set) => ({
  sessionsById: new Map(),
  recentlyClosed: [],
  summary: null,
  connectionState: 'disconnected',
  isInitialized: false,
  lastUpdateTs: 0,

  handleSnapshot: (summary, sessions, recentlyClosed) => {
    const map = new Map<string, LiveSession>()
    const now = Date.now()
    _eventTimes.clear()
    for (const s of sessions) {
      map.set(s.id, s)
      _eventTimes.set(s.id, now)
    }
    set({
      sessionsById: map,
      recentlyClosed,
      summary,
      isInitialized: true,
      lastUpdateTs: now,
    })
  },

  handleUpsert: (session) => {
    _eventTimes.set(session.id, Date.now())
    set((state) => {
      const next = new Map(state.sessionsById)
      next.set(session.id, session)
      return { sessionsById: next, lastUpdateTs: Date.now() }
    })
  },

  handleRemove: (sessionId, session) => {
    _eventTimes.delete(sessionId)
    set((state) => {
      const next = new Map(state.sessionsById)
      next.delete(sessionId)
      // Prepend to closed list, cap at 100. Avoid spread when at capacity —
      // splice is O(1) amortized vs spread's O(n) copy.
      const closed =
        state.recentlyClosed.length >= 100
          ? [session, ...state.recentlyClosed.slice(0, 99)]
          : [session, ...state.recentlyClosed]
      return { sessionsById: next, recentlyClosed: closed, lastUpdateTs: Date.now() }
    })
  },

  setConnectionState: (connectionState) => set({ connectionState }),

  dismissSession: async (sessionId) => {
    set((state) => ({ recentlyClosed: state.recentlyClosed.filter((s) => s.id !== sessionId) }))
    try {
      await fetch(`/api/live/sessions/${sessionId}/dismiss`, { method: 'DELETE' })
    } catch {
      /* best-effort */
    }
  },

  dismissAllClosed: async () => {
    set({ recentlyClosed: [] })
    try {
      await fetch('/api/live/recently-closed', { method: 'DELETE' })
    } catch {
      /* best-effort */
    }
  },
}))

// --- Selectors ---

export function useActiveSessions(): LiveSession[] {
  const sessionsById = useLiveSessionStore((s) => s.sessionsById)
  return useMemo(() => {
    const arr = Array.from(sessionsById.values())
    arr.sort((a, b) => b.lastActivityAt - a.lastActivityAt)
    return arr
  }, [sessionsById])
}

export function useRecentlyClosed(): LiveSession[] {
  return useLiveSessionStore((s) => s.recentlyClosed)
}

export function useLiveSummary(): LiveSummary | null {
  return useLiveSessionStore((s) => s.summary)
}

export function useIsLiveConnected(): boolean {
  return useLiveSessionStore((s) => s.connectionState === 'connected')
}

export function useIsLiveInitialized(): boolean {
  return useLiveSessionStore((s) => s.isInitialized)
}

/**
 * Derive stalled sessions from the mutable event-time map.
 * Driven by a caller-provided `tick` (e.g. a 1s timer) so it re-evaluates
 * periodically without Zustand reactivity.
 */
export function useStalledSessions(tick: number): Set<string> {
  // biome-ignore lint/correctness/useExhaustiveDependencies: tick is an intentional external re-evaluation trigger (1s timer), not a reactive dependency
  return useMemo(() => {
    const now = Date.now()
    const stalled = new Set<string>()
    for (const [id, lastTime] of _eventTimes.entries()) {
      if (now - lastTime > 3000) stalled.add(id)
    }
    return stalled
  }, [tick])
}
