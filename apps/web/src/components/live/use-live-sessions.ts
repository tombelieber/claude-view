import { useEffect } from 'react'
import { useLiveSessionStore } from '../../store/live-session-store'
import { sseUrl } from '../../lib/sse-url'

/**
 * Pure SSE transport — opens EventSource, routes events into the zustand store.
 * Call once at the app root (App.tsx). Stateless: all state lives in useLiveSessionStore.
 */
export function useLiveSSE(): void {
  // biome-ignore lint/correctness/useExhaustiveDependencies: useLiveSessionStore is a stable module-level zustand store singleton, not a React value
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
          useLiveSessionStore.getState().setConnectionState('connected')
          retryDelay = 1000
        }
      }

      es.addEventListener('snapshot', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          useLiveSessionStore
            .getState()
            .handleSnapshot(data.summary ?? {}, data.sessions ?? [], data.recentlyClosed ?? [])
        } catch {
          /* ignore malformed */
        }
      })

      es.addEventListener('session_upsert', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const session = data.session ?? data
          if (session?.id) useLiveSessionStore.getState().handleUpsert(session)
        } catch {
          /* ignore */
        }
      })

      es.addEventListener('session_remove', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          if (data.sessionId && data.session)
            useLiveSessionStore.getState().handleRemove(data.sessionId, data.session)
        } catch {
          /* ignore */
        }
      })

      // Forward CLI session events as window CustomEvents so useCliSessions
      // can update its React Query cache without opening a second EventSource.
      for (const eventName of [
        'cli_session_created',
        'cli_session_updated',
        'cli_session_removed',
      ] as const) {
        es.addEventListener(eventName, (e: MessageEvent) => {
          try {
            window.dispatchEvent(new CustomEvent(`cv:${eventName}`, { detail: JSON.parse(e.data) }))
          } catch {
            /* malformed */
          }
        })
      }

      es.onerror = () => {
        if (unmounted) return
        useLiveSessionStore.getState().setConnectionState('reconnecting')
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
}

// --- Re-exports for backward compatibility ---
// Many files import LiveSession, LiveSummary, sessionTotalCost from this file.
// Keep these re-exports so consumers don't need to change their import paths.

export type { LiveSession } from '@claude-view/shared/types/generated'
export type { LiveSummary } from '../../store/live-session-store'
export {
  useActiveSessions,
  useRecentlyClosed,
  useLiveSummary,
  useIsLiveConnected,
  useIsLiveInitialized,
  useLiveSessionStore,
} from '../../store/live-session-store'

export function sessionTotalCost(
  session: import('@claude-view/shared/types/generated').LiveSession,
): number {
  const subAgentTotal = session.subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  return (session.cost?.totalUsd ?? 0) + subAgentTotal
}
