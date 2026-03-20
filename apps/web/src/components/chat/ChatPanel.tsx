import type { IDockviewPanelProps } from 'dockview-react'
import { useCallback, useEffect, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import type { LiveStatus } from '../../lib/derive-panel-mode'
import { ChatSession } from '../../pages/ChatSession'

interface ChatPanelParams {
  sessionId: string
  liveStatus?: LiveStatus
}

/**
 * Thin dockview wrapper — delegates all rendering to ChatSession (V1).
 * The V2 revamp is architectural (direct TCP, multi-session dockview),
 * not a UI rewrite. ChatSession owns all rich rendering, scroll anchoring,
 * permission handling, model selection, and command palette.
 */
export function ChatPanel({ params, api }: IDockviewPanelProps<ChatPanelParams>) {
  const { sessionId, liveStatus } = params
  const containerRef = useRef<HTMLDivElement>(null)
  const navigate = useNavigate()

  // Dockview panels may not have final dimensions when ChatSession's
  // useScrollAnchor fires its initial scroll-to-bottom. Retry after
  // the panel layout settles to ensure we start at the bottom.
  useEffect(() => {
    if (!sessionId) return
    const timer = setTimeout(() => {
      const scroller = containerRef.current?.querySelector('[class*="overflow-y-auto"]')
      if (scroller) scroller.scrollTop = scroller.scrollHeight
    }, 300)
    return () => clearTimeout(timer)
  }, [sessionId])

  // Called when ChatSession creates a new session from the blank "New Chat" panel.
  // Transitions this panel from blank to the real session.
  const onSessionCreated = useCallback(
    (newSessionId: string) => {
      api.updateParameters({ sessionId: newSessionId })
      api.setTitle(newSessionId.slice(0, 8))
      navigate(`/chat/${newSessionId}`)
    },
    [api, navigate],
  )

  return (
    <div ref={containerRef} className="flex flex-col h-full overflow-hidden">
      <ChatSession
        sessionId={sessionId || undefined}
        liveStatus={liveStatus ?? 'inactive'}
        onSessionCreated={!sessionId ? onSessionCreated : undefined}
      />
    </div>
  )
}
