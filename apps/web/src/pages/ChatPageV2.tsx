import type { DockviewApi } from 'dockview-react'
import { useCallback, useState } from 'react'
import { ChatDockLayout } from '../components/chat/ChatDockLayout'
import { useChatKeyboardShortcuts } from '../hooks/use-chat-keyboard-shortcuts'

export function ChatPageV2() {
  const [dockApi, setDockApi] = useState<DockviewApi | null>(null)

  useChatKeyboardShortcuts(dockApi)

  const handleDockReady = useCallback((api: DockviewApi) => {
    setDockApi(api)
  }, [])

  const openSession = useCallback(
    (sessionId: string, title?: string) => {
      if (!dockApi) return
      // Check if panel already exists
      const existing = dockApi.panels.find(
        (p) => (p.params as { sessionId?: string })?.sessionId === sessionId,
      )
      if (existing) {
        existing.api.setActive()
        return
      }
      dockApi.addPanel({
        id: `chat-${sessionId}`,
        component: 'chat',
        title: title ?? sessionId.slice(0, 8),
        params: { sessionId },
      })
    },
    [dockApi],
  )

  // openSession will be wired to SessionSidebar in Phase 3.
  void openSession

  return (
    <div className="flex h-full">
      {/* SessionSidebar integration will happen in Phase 3 */}
      <div className="flex-1 flex flex-col">
        <ChatDockLayout onReady={handleDockReady} />
      </div>
    </div>
  )
}
