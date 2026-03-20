import type { IDockviewPanelProps } from 'dockview-react'
import { ChatSession } from '../../pages/ChatSession'

interface ChatPanelParams {
  sessionId: string
  isWatching?: boolean
}

/**
 * Thin dockview wrapper — delegates all rendering to ChatSession (V1).
 * The V2 revamp is architectural (direct TCP, multi-session dockview),
 * not a UI rewrite. ChatSession owns all rich rendering, scroll anchoring,
 * permission handling, model selection, and command palette.
 */
export function ChatPanel({ params }: IDockviewPanelProps<ChatPanelParams>) {
  const { sessionId, isWatching } = params

  return <ChatSession sessionId={sessionId || undefined} isWatching={isWatching} />
}
