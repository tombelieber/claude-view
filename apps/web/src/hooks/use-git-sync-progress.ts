import { useState, useEffect } from 'react'

export type GitSyncPhase = 'idle' | 'scanning' | 'correlating' | 'done' | 'error'

export interface GitSyncProgress {
  phase: GitSyncPhase
  reposScanned: number
  totalRepos: number
  commitsFound: number
  sessionsCorrelated: number
  totalCorrelatableSessions: number
  linksCreated: number
  errorMessage?: string
}

const INITIAL_STATE: GitSyncProgress = {
  phase: 'idle',
  reposScanned: 0,
  totalRepos: 0,
  commitsFound: 0,
  sessionsCorrelated: 0,
  totalCorrelatableSessions: 0,
  linksCreated: 0,
}

/**
 * SSE endpoint URL.
 * In dev mode (Vite on :5173), bypass the proxy and hit the Rust server directly —
 * Vite's http-proxy buffers SSE, defeating real-time feedback.
 */
function sseUrl(): string {
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return 'http://localhost:47892/api/sync/git/progress'
  }
  return '/api/sync/git/progress'
}

/**
 * Hook that streams git sync progress via SSE from `GET /api/sync/git/progress`.
 *
 * Only connects when `enabled` is true (after the user triggers a git sync).
 * Automatically closes on completion, error, or unmount.
 */
export function useGitSyncProgress(enabled: boolean): GitSyncProgress {
  const [progress, setProgress] = useState<GitSyncProgress>(INITIAL_STATE)

  useEffect(() => {
    if (!enabled) return

    setProgress(INITIAL_STATE)

    const es = new EventSource(sseUrl())

    es.addEventListener('scanning', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Received malformed progress data from server',
        })
        es.close()
        return
      }
      setProgress((prev) => ({
        ...prev,
        phase: 'scanning',
        reposScanned: data.reposScanned ?? 0,
        totalRepos: data.totalRepos ?? 0,
        commitsFound: data.commitsFound ?? 0,
      }))
    })

    es.addEventListener('correlating', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Received malformed progress data from server',
        })
        es.close()
        return
      }
      setProgress((prev) => ({
        ...prev,
        phase: 'correlating',
        sessionsCorrelated: data.sessionsCorrelated ?? 0,
        totalCorrelatableSessions: data.totalCorrelatableSessions ?? 0,
        commitsFound: data.commitsFound ?? 0,
        linksCreated: data.linksCreated ?? 0,
      }))
    })

    es.addEventListener('done', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Received malformed progress data from server',
        })
        es.close()
        return
      }
      setProgress((prev) => ({
        ...prev,
        phase: 'done',
        reposScanned: data.reposScanned ?? 0,
        commitsFound: data.commitsFound ?? 0,
        linksCreated: data.linksCreated ?? 0,
      }))
      es.close()
    })

    // Server-sent error events (event: error\ndata: {...}) arrive as MessageEvents
    // with data. Browser connection errors arrive as plain Events without data.
    es.addEventListener('error', (e: Event) => {
      if ('data' in e && (e as MessageEvent).data) {
        let data
        try {
          data = JSON.parse((e as MessageEvent).data)
        } catch {
          setProgress({
            ...INITIAL_STATE,
            phase: 'error',
            errorMessage: 'Received malformed progress data from server',
          })
          es.close()
          return
        }
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: data.message ?? 'Unknown error',
        })
      } else if (es.readyState === EventSource.CLOSED) {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Lost connection to server',
        })
      } else {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Connection to server failed',
        })
      }
      es.close()
    })

    return () => es.close()
  }, [enabled])

  return progress
}
