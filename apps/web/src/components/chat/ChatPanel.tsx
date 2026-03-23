import type { IDockviewPanelProps } from 'dockview-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { LiveStatus } from '../../lib/live-status'
import { ChatSession } from '../../pages/ChatSession'

interface ChatPanelParams {
  sessionId: string
  liveStatus?: LiveStatus
  liveProjectPath?: string
}

/**
 * Thin dockview wrapper — delegates all rendering to ChatSession (V1).
 * The V2 revamp is architectural (direct TCP, multi-session dockview),
 * not a UI rewrite. ChatSession owns all rich rendering, scroll anchoring,
 * permission handling, model selection, and command palette.
 */
export function ChatPanel({ params, api }: IDockviewPanelProps<ChatPanelParams>) {
  const { sessionId, liveStatus, liveProjectPath } = params
  const containerRef = useRef<HTMLDivElement>(null)
  const navigate = useNavigate()

  // Focus the chat textarea when this panel becomes active (tab switch, sidebar click).
  // Uses dockview's PanelApi.onDidActiveChange — fires when tab is selected.
  useEffect(() => {
    const focus = () => {
      requestAnimationFrame(() => {
        const textarea = containerRef.current?.querySelector<HTMLTextAreaElement>(
          '[data-testid="chat-input"]',
        )
        if (textarea && !textarea.disabled) textarea.focus()
      })
    }

    // Focus on initial mount if this panel is already active
    if (api.isActive) focus()

    // Focus on subsequent tab switches
    const disposable = api.onDidActiveChange((event) => {
      if (event.isActive) focus()
    })
    return () => disposable.dispose()
  }, [api])

  // Scroll-to-bottom signal: incremented when dockview moves this panel
  // to a different group (drag-drop). The DOM is reparented without a React
  // remount — scrollTop resets to 0 but no lifecycle fires. This counter
  // propagates through ChatSession → ConversationThread to trigger scroll.
  const [scrollSignal, setScrollSignal] = useState(0)
  useEffect(() => {
    const disposable = api.onDidGroupChange(() => {
      setScrollSignal((n) => n + 1)
    })
    return () => disposable.dispose()
  }, [api])

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
    <div ref={containerRef} className="flex flex-col h-full min-w-0 overflow-hidden">
      <ChatSession
        sessionId={sessionId || undefined}
        liveStatus={liveStatus ?? 'inactive'}
        liveProjectPath={liveProjectPath}
        onSessionCreated={!sessionId ? onSessionCreated : undefined}
        scrollToBottomSignal={scrollSignal}
      />
    </div>
  )
}
