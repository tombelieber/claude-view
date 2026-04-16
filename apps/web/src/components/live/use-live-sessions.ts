import { useEffect } from 'react'
import { useLiveSessionStore } from '../../store/live-session-store'
import { sseUrl } from '../../lib/sse-url'
import type { LiveSession } from '@claude-view/shared/types/generated'
import type { LiveSummary } from '../../store/live-session-store'

// ---------------------------------------------------------------------------
// SSE Event Batcher
//
// Rapid session events (10+ removes in a burst) previously caused 10
// sequential Zustand state updates, each triggering a full React re-render.
// This batcher collects events within a single animation frame and commits
// them as ONE store transaction, capping re-renders to 1-per-frame (~60Hz).
// ---------------------------------------------------------------------------

type SSEEvent =
  | {
      type: 'snapshot'
      summary: LiveSummary
      sessions: LiveSession[]
      recentlyClosed: LiveSession[]
    }
  | { type: 'upsert'; session: LiveSession }
  | { type: 'remove'; sessionId: string; session: LiveSession }

let _pendingEvents: SSEEvent[] = []
let _rafId: number | null = null

function enqueue(event: SSEEvent): void {
  _pendingEvents.push(event)
  if (_rafId === null) {
    _rafId = requestAnimationFrame(flush)
  }
}

function flush(): void {
  _rafId = null
  const batch = _pendingEvents
  _pendingEvents = []
  if (batch.length === 0) return

  const store = useLiveSessionStore.getState()

  for (const event of batch) {
    switch (event.type) {
      case 'snapshot':
        store.handleSnapshot(event.summary, event.sessions, event.recentlyClosed)
        break
      case 'upsert':
        store.handleUpsert(event.session)
        break
      case 'remove':
        store.handleRemove(event.sessionId, event.session)
        break
    }
  }
}

// ---------------------------------------------------------------------------
// SSE Transport Hook
// ---------------------------------------------------------------------------

/**
 * Pure SSE transport — opens EventSource, routes events into the zustand store.
 * Call once at the app root (App.tsx). Stateless: all state lives in useLiveSessionStore.
 */
export function useLiveSSE(): void {
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
          // Snapshots bypass the batcher — they replace all state, so
          // batching with incremental events would produce wrong results.
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
          if (session?.id) enqueue({ type: 'upsert', session })
        } catch {
          /* ignore */
        }
      })

      es.addEventListener('session_remove', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          if (data.sessionId && data.session)
            enqueue({ type: 'remove', sessionId: data.sessionId, session: data.session })
        } catch {
          /* ignore */
        }
      })

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
      // Flush any pending events before unmount
      if (_rafId !== null) {
        cancelAnimationFrame(_rafId)
        _rafId = null
      }
      _pendingEvents = []
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
