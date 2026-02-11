import { useState, useEffect, useMemo } from 'react'
import { sseUrl } from '../lib/sse-url'

export interface LiveSession {
  id: string
  project: string
  projectDisplayName: string
  projectPath: string
  filePath: string
  status: 'streaming' | 'tool_use' | 'waiting_for_user' | 'idle' | 'complete'
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
  }
  cacheStatus: 'warm' | 'cold' | 'unknown'
}

export interface LiveSummary {
  activeCount: number
  waitingCount: number
  idleCount: number
  totalCostTodayUsd: number
  totalTokensToday: number
}

export interface UseLiveSessionsResult {
  sessions: LiveSession[]
  summary: LiveSummary | null
  isConnected: boolean
  lastUpdate: Date | null
}

export function useLiveSessions(): UseLiveSessionsResult {
  const [sessions, setSessions] = useState<Map<string, LiveSession>>(new Map())
  const [summary, setSummary] = useState<LiveSummary | null>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null)

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
          }
        } catch { /* ignore */ }
      })

      es.addEventListener('session_completed', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const sessionId = data.session_id ?? data.sessionId
          if (sessionId) {
            setSessions(prev => {
              const next = new Map(prev)
              next.delete(sessionId)
              return next
            })
            setLastUpdate(new Date())
          }
        } catch { /* ignore */ }
      })

      es.addEventListener('summary', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          // Handle both wrapped and unwrapped formats
          const s = data.activeCount !== undefined ? data : data.summary ?? data
          setSummary(s)
          setLastUpdate(new Date())
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

  const sessionList = useMemo(
    () => Array.from(sessions.values()).sort((a, b) => b.lastActivityAt - a.lastActivityAt),
    [sessions]
  )

  return { sessions: sessionList, summary, isConnected, lastUpdate }
}
