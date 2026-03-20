import { useQuery } from '@tanstack/react-query'
import type { DockviewApi } from 'dockview-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useOutletContext, useParams } from 'react-router-dom'
import { ChatDockLayout } from '../components/chat/ChatDockLayout'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { useChatKeyboardShortcuts } from '../hooks/use-chat-keyboard-shortcuts'

export function ChatPageV2() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()

  const [dockApi, setDockApi] = useState<DockviewApi | null>(null)
  const dockApiRef = useRef<DockviewApi | null>(null)

  useChatKeyboardShortcuts(dockApi)

  // Poll sidecar-managed sessions (same pattern as old ChatPage)
  const { data: sidecarIds } = useQuery({
    queryKey: ['sidecar-sessions'],
    queryFn: async () => {
      const res = await fetch('/api/sidecar/sessions')
      if (!res.ok) return new Set<string>()
      const sessions: { sessionId: string }[] = await res.json()
      return new Set(sessions.map((s) => s.sessionId))
    },
    refetchInterval: 5_000,
  })

  const handleDockReady = useCallback((api: DockviewApi) => {
    setDockApi(api)
    dockApiRef.current = api
  }, [])

  const openSession = useCallback(
    (sid: string, title?: string) => {
      const api = dockApiRef.current
      if (!api) return
      const existing = api.panels.find(
        (p) => (p.params as { sessionId?: string })?.sessionId === sid,
      )
      if (existing) {
        existing.api.setActive()
        return
      }
      // Determine if this is a watching session
      const isLiveElsewhere = liveSessions.sessions.some((s) => s.id === sid)
      const isSidecarManaged = sidecarIds == null || sidecarIds.has(sid)
      const isWatching = isLiveElsewhere && !isSidecarManaged

      api.addPanel({
        id: `chat-${sid}`,
        component: 'chat',
        title: title ?? sid.slice(0, 8),
        params: { sessionId: sid, isWatching },
      })
    },
    [liveSessions.sessions, sidecarIds],
  )

  // Auto-open panel when URL has sessionId (sidebar navigates via React Router)
  useEffect(() => {
    if (!sessionId || !dockApiRef.current) return
    openSession(sessionId)
  }, [sessionId, openSession])

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar liveSessions={liveSessions.sessions} sidecarSessionIds={sidecarIds} />
      <div className="flex-1 flex flex-col">
        <ChatDockLayout onReady={handleDockReady} />
      </div>
    </div>
  )
}
