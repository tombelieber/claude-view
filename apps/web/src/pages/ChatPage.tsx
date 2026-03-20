import { useOutletContext, useParams } from 'react-router-dom'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { ChatSession } from './ChatSession'

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()

  // Derive isWatching from SSE LiveSession.control field — no polling needed.
  // Live elsewhere + no sidecar control binding = watching (read-only).
  const liveSession = liveSessions.sessions.find((s) => s.id === sessionId)
  const isWatching = liveSession != null && liveSession.control == null

  // Pass authoritative context gauge data from Live Monitor SSE.
  // Statusline values are ground truth — computed by Claude Code itself.
  const liveContextData = liveSession
    ? {
        contextWindowTokens: liveSession.contextWindowTokens,
        statuslineContextWindowSize: liveSession.statuslineContextWindowSize ?? null,
        statuslineUsedPct: liveSession.statuslineUsedPct ?? null,
      }
    : undefined

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar liveSessions={liveSessions.sessions} />
      <ChatSession
        key={sessionId ?? 'new'}
        sessionId={sessionId}
        isWatching={isWatching}
        liveContextData={liveContextData}
      />
    </div>
  )
}
