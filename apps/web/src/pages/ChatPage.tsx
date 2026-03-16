import { useOutletContext, useParams } from 'react-router-dom'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { ChatSession } from './ChatSession'

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()

  // A session is spectating if it's live AND the user did NOT explicitly
  // initiate it (create/resume/fork set location.state). Clicking a session
  // in the sidebar = spectating. This avoids relying on the server's `control`
  // field which can be stale from previous WS connections.
  const isLiveElsewhere = liveSessions.sessions.some((s) => s.id === sessionId)

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar liveSessions={liveSessions.sessions} />
      <ChatSession
        key={sessionId ?? 'new'}
        sessionId={sessionId}
        isLiveElsewhere={isLiveElsewhere}
      />
    </div>
  )
}
