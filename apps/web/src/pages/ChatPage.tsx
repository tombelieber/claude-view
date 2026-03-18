import { useQuery } from '@tanstack/react-query'
import { useOutletContext, useParams } from 'react-router-dom'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { ChatSession } from './ChatSession'

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()

  // Sidecar-managed sessions: created/resumed through Claude View chat.
  // Polls every 5s so the sidebar picks up newly-created sessions without
  // depending on liveSessions count changes.
  const { data: sidecarIds } = useQuery({
    queryKey: ['sidecar-sessions'],
    queryFn: async () => {
      const res = await fetch('/api/control/sessions')
      if (!res.ok) return new Set<string>()
      const sessions: { sessionId: string }[] = await res.json()
      return new Set(sessions.map((s) => s.sessionId))
    },
    refetchInterval: 5_000,
  })

  // Watching = session is live (detected by hooks/SSE) but NOT managed by our sidecar.
  // When sidecarIds is still loading (undefined), default to NOT watching to avoid
  // blocking WS connections for the user's own sessions.
  const isLiveElsewhere = liveSessions.sessions.some((s) => s.id === sessionId)
  const isSidecarManaged = sidecarIds == null || sidecarIds.has(sessionId ?? '')
  const isWatching = isLiveElsewhere && !isSidecarManaged

  // Pass authoritative context gauge data from Live Monitor SSE.
  // Statusline values are ground truth — computed by Claude Code itself.
  const liveSession = liveSessions.sessions.find((s) => s.id === sessionId)
  const liveContextData = liveSession
    ? {
        contextWindowTokens: liveSession.contextWindowTokens,
        statuslineContextWindowSize: liveSession.statuslineContextWindowSize ?? null,
        statuslineUsedPct: liveSession.statuslineUsedPct ?? null,
      }
    : undefined

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar liveSessions={liveSessions.sessions} sidecarSessionIds={sidecarIds} />
      <ChatSession
        key={sessionId ?? 'new'}
        sessionId={sessionId}
        isWatching={isWatching}
        liveContextData={liveContextData}
      />
    </div>
  )
}
