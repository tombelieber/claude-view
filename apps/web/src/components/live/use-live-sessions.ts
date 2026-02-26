import { useState, useEffect, useMemo, useRef } from 'react'
import { sseUrl } from '../../lib/sse-url'
import type { AgentState } from './types'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import type { ProgressItem } from '../../types/generated/ProgressItem'

const STALL_THRESHOLD_MS = 3000

export interface LiveSession {
  id: string
  project: string
  projectDisplayName: string
  projectPath: string
  filePath: string
  status: 'working' | 'paused' | 'done'
  agentState: AgentState
  gitBranch: string | null
  pid: number | null
  title: string
  lastUserMessage: string
  currentActivity: string
  turnCount: number
  startedAt: number | null
  lastActivityAt: number
  model: string | null
  tokens: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheCreationTokens: number
    totalTokens: number
  }
  contextWindowTokens: number
  cost: {
    totalUsd: number
    inputCostUsd: number
    outputCostUsd: number
    cacheReadCostUsd: number
    cacheCreationCostUsd: number
    cacheSavingsUsd: number
    isEstimated: boolean
  }
  cacheStatus: 'warm' | 'cold' | 'unknown'
  currentTurnStartedAt?: number | null
  lastTurnTaskSeconds?: number | null
  subAgents?: SubAgentInfo[]
  progressItems?: ProgressItem[]
  lastCacheHitAt?: number | null
  toolsUsed?: { name: string; kind: 'mcp' | 'skill' }[]
}

export interface LiveSummary {
  needsYouCount: number
  autonomousCount: number
  totalCostTodayUsd: number
  totalTokensToday: number
  processCount: number
}

export interface UseLiveSessionsResult {
  sessions: LiveSession[]
  summary: LiveSummary | null
  isConnected: boolean
  /** True after the first SSE summary event arrives (server has done its initial scan) */
  isInitialized: boolean
  lastUpdate: Date | null
  /** Session IDs with no SSE event for >3 seconds */
  stalledSessions: Set<string>
  /** Unix epoch seconds, ticks every ~1s for duration computation */
  currentTime: number
}

export function sessionTotalCost(session: LiveSession): number {
  const subAgentTotal = session.subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  return (session.cost?.totalUsd ?? 0) + subAgentTotal
}

export function useLiveSessions(): UseLiveSessionsResult {
  const [sessions, setSessions] = useState<Map<string, LiveSession>>(new Map())
  const [summary, setSummary] = useState<LiveSummary | null>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [isInitialized, setIsInitialized] = useState(false)
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null)
  const lastEventTimes = useRef<Map<string, number>>(new Map())
  const [stalledSessions, setStalledSessions] = useState<Set<string>>(new Set())
  const [currentTime, setCurrentTime] = useState<number>(() => Math.floor(Date.now() / 1000))
  const resyncRef = useRef<{ ids: Set<string>; timer: ReturnType<typeof setTimeout> | null } | null>(null)

  useEffect(() => {
    let es: EventSource | null = null
    let retryDelay = 1000
    let unmounted = false

    function connect() {
      if (unmounted) return

      const url = sseUrl('/api/live/stream')
      es = new EventSource(url)

      es.onopen = () => {
        if (!unmounted) {
          setIsConnected(true)
          retryDelay = 1000
        }
      }

      es.addEventListener('session_discovered', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const session = data.session ?? data
          if (session?.id) {
            setSessions(prev => new Map(prev).set(session.id, session))
            setLastUpdate(new Date())
            lastEventTimes.current.set(session.id, Date.now())
            // Track for resync window
            if (resyncRef.current) resyncRef.current.ids.add(session.id)
          }
        } catch { /* ignore malformed */ }
      })

      es.addEventListener('session_updated', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const session = data.session ?? data
          if (session?.id) {
            setSessions(prev => new Map(prev).set(session.id, session))
            setLastUpdate(new Date())
            lastEventTimes.current.set(session.id, Date.now())
          }
        } catch { /* ignore */ }
      })

      es.addEventListener('session_completed', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          if (data.sessionId) {
            setSessions(prev => {
              const next = new Map(prev)
              next.delete(data.sessionId)
              return next
            })
            lastEventTimes.current.delete(data.sessionId)
            setLastUpdate(new Date())
          }
        } catch { /* ignore */ }
      })

      es.addEventListener('summary', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          // Backend always sends summary fields at top level (needsYouCount, etc.)
          setSummary(data)
          setIsInitialized(true)
          setLastUpdate(new Date())

          // After a lag recovery, the server re-sends all active sessions
          // as session_discovered events. Track which IDs arrive in the
          // next batch so we can prune sessions that no longer exist.
          resyncRef.current = { ids: new Set<string>(), timer: null }
          resyncRef.current.timer = window.setTimeout(() => {
            if (resyncRef.current) {
              const validIds = resyncRef.current.ids
              if (validIds.size > 0) {
                setSessions(prev => {
                  const next = new Map<string, LiveSession>()
                  for (const [id, session] of prev) {
                    if (validIds.has(id)) next.set(id, session)
                  }
                  return next
                })
              }
              resyncRef.current = null
            }
          }, 500) // 500ms window for all session_discovered to arrive
        } catch { /* ignore */ }
      })

      es.onerror = () => {
        if (unmounted) return
        setIsConnected(false)
        es?.close()
        setTimeout(connect, retryDelay)
        retryDelay = Math.min(retryDelay * 2, 30000)
      }
    }

    connect()

    return () => {
      unmounted = true
      es?.close()
    }
  }, [])

  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now()
      // Stall detection: only update state when the stalled set actually changes
      setStalledSessions(prev => {
        const stalled = new Set<string>()
        for (const [id, lastTime] of lastEventTimes.current.entries()) {
          if (now - lastTime > STALL_THRESHOLD_MS) stalled.add(id)
        }
        if (stalled.size === prev.size && [...stalled].every(id => prev.has(id))) return prev
        return stalled
      })
      // Clock tick for duration computation (shared across all cards)
      setCurrentTime(Math.floor(now / 1000))
    }, 1000)
    return () => clearInterval(interval)
  }, [])

  const sessionList = useMemo(
    () => Array.from(sessions.values()).sort((a, b) => b.lastActivityAt - a.lastActivityAt),
    [sessions]
  )

  return { sessions: sessionList, summary, isConnected, isInitialized, lastUpdate, stalledSessions, currentTime }
}
