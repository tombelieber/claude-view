import type { LiveSession } from '@claude-view/shared/types/generated'
export type { LiveSession }
import { useEffect, useMemo, useRef, useState } from 'react'
import { sseUrl } from '../../lib/sse-url'

const STALL_THRESHOLD_MS = 3000

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

export interface UseLiveSessionsResult {
  sessions: LiveSession[]
  summary: LiveSummary | null
  isConnected: boolean
  isInitialized: boolean
  lastUpdate: Date | null
  stalledSessions: Set<string>
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
  const resyncRef = useRef<{
    ids: Set<string>
    timer: number | null
  } | null>(null)

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
            setSessions((prev) => new Map(prev).set(session.id, session))
            setLastUpdate(new Date())
            lastEventTimes.current.set(session.id, Date.now())
            if (resyncRef.current) resyncRef.current.ids.add(session.id)
          }
        } catch {
          /* ignore malformed */
        }
      })

      es.addEventListener('session_updated', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const session = data.session ?? data
          if (session?.id) {
            setSessions((prev) => new Map(prev).set(session.id, session))
            setLastUpdate(new Date())
            lastEventTimes.current.set(session.id, Date.now())
          }
        } catch {
          /* ignore */
        }
      })

      es.addEventListener('session_completed', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          if (data.sessionId) {
            setSessions((prev) => {
              const next = new Map(prev)
              next.delete(data.sessionId)
              return next
            })
            lastEventTimes.current.delete(data.sessionId)
            setLastUpdate(new Date())
          }
        } catch {
          /* ignore */
        }
      })

      es.addEventListener('summary', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          setSummary({
            needsYouCount: data.needsYouCount ?? 0,
            autonomousCount: data.autonomousCount ?? 0,
            totalCostTodayUsd: data.totalCostTodayUsd ?? 0,
            totalTokensToday: data.totalTokensToday ?? 0,
            processCount: data.processCount ?? 0,
            inputTokens: data.inputTokens ?? 0,
            outputTokens: data.outputTokens ?? 0,
            cacheReadTokens: data.cacheReadTokens ?? 0,
            cacheCreationTokens: data.cacheCreationTokens ?? 0,
            inputCostUsd: data.inputCostUsd ?? 0,
            outputCostUsd: data.outputCostUsd ?? 0,
            cacheReadCostUsd: data.cacheReadCostUsd ?? 0,
            cacheCreationCostUsd: data.cacheCreationCostUsd ?? 0,
            cacheSavingsUsd: data.cacheSavingsUsd ?? 0,
          })
          setIsInitialized(true)
          setLastUpdate(new Date())

          resyncRef.current = { ids: new Set<string>(), timer: null }
          resyncRef.current.timer = window.setTimeout(() => {
            if (resyncRef.current) {
              const validIds = resyncRef.current.ids
              if (validIds.size > 0) {
                setSessions((prev) => {
                  const next = new Map<string, LiveSession>()
                  for (const [id, session] of prev) {
                    if (validIds.has(id)) next.set(id, session)
                  }
                  return next
                })
              }
              resyncRef.current = null
            }
          }, 500)
        } catch {
          /* ignore */
        }
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
      setStalledSessions((prev) => {
        const stalled = new Set<string>()
        for (const [id, lastTime] of lastEventTimes.current.entries()) {
          if (now - lastTime > STALL_THRESHOLD_MS) stalled.add(id)
        }
        if (stalled.size === prev.size && [...stalled].every((id) => prev.has(id))) return prev
        return stalled
      })
      setCurrentTime(Math.floor(now / 1000))
    }, 1000)
    return () => clearInterval(interval)
  }, [])

  const sessionList = useMemo(
    () => Array.from(sessions.values()).sort((a, b) => b.lastActivityAt - a.lastActivityAt),
    [sessions],
  )

  return {
    sessions: sessionList,
    summary,
    isConnected,
    isInitialized,
    lastUpdate,
    stalledSessions,
    currentTime,
  }
}
