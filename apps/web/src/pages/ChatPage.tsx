import { useOutletContext, useParams } from 'react-router-dom'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { ChatSession } from './ChatSession'

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()

  // Spectating = session is live but NOT controlled by our SDK (running in CLI/VS Code/etc.)
  const liveMatch = liveSessions.sessions.find((s) => s.id === sessionId)
  const isSpectating = !!liveMatch && liveMatch.control === null

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar liveSessions={liveSessions.sessions} />
      <ChatSession key={sessionId ?? 'new'} sessionId={sessionId} isSpectating={isSpectating} />
    </div>
  )
}
