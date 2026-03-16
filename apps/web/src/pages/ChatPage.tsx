import { useParams } from 'react-router-dom'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import { ChatSession } from './ChatSession'

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId?: string }>()

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar />
      <ChatSession key={sessionId ?? 'new'} sessionId={sessionId} />
    </div>
  )
}
