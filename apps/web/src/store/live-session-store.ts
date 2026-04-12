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

interface LiveSessionState {
  sessionsById: Map<string, LiveSession>
  recentlyClosed: LiveSession[]
  summary: LiveSummary | null
  connectionState: ConnectionState
  isInitialized: boolean
  lastUpdate: Date | null
  lastEventTimes: Map<string, number>

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
  lastUpdate: null,
  lastEventTimes: new Map(),

  handleSnapshot: (summary, sessions, recentlyClosed) => {
    const map = new Map<string, LiveSession>()
    for (const s of sessions) map.set(s.id, s)
    const now = Date.now()
    const times = new Map<string, number>()
    for (const s of sessions) times.set(s.id, now)
    set({
      sessionsById: map,
      recentlyClosed,
      summary,
      isInitialized: true,
      lastUpdate: new Date(),
      lastEventTimes: times,
    })
  },

  handleUpsert: (session) => {
    set((state) => {
      const next = new Map(state.sessionsById)
      next.set(session.id, session)
      const times = new Map(state.lastEventTimes)
      times.set(session.id, Date.now())
      return { sessionsById: next, lastUpdate: new Date(), lastEventTimes: times }
    })
  },

  handleRemove: (sessionId, session) => {
    set((state) => {
      const next = new Map(state.sessionsById)
      next.delete(sessionId)
      const times = new Map(state.lastEventTimes)
      times.delete(sessionId)
      const closed = [session, ...state.recentlyClosed].slice(0, 100)
      return {
        sessionsById: next,
        recentlyClosed: closed,
        lastUpdate: new Date(),
        lastEventTimes: times,
      }
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
