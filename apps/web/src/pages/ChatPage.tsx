import { useOutletContext, useParams } from 'react-router-dom'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { deriveLiveStatus } from '../lib/live-status'
import { ChatSession } from './ChatSession'

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()

  // Derive liveStatus from SSE LiveSession.control field — no polling needed.
  const liveSession = liveSessions.sessions.find((s) => s.id === sessionId)
  const liveStatus = deriveLiveStatus(liveSession)

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
        liveStatus={liveStatus}
        liveContextData={liveContextData}
      />
    </div>
  )
}
