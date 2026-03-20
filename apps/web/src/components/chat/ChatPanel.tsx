import type { IDockviewPanelProps } from 'dockview-react'
import { useEffect, useRef } from 'react'
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
  const containerRef = useRef<HTMLDivElement>(null)

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

  return (
    <div ref={containerRef} className="flex flex-col h-full overflow-hidden">
      <ChatSession sessionId={sessionId || undefined} isWatching={isWatching} />
    </div>
  )
}
