import type { DockviewApi } from 'dockview-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useOutletContext, useParams } from 'react-router-dom'
import { ChatDockLayout } from '../components/chat/ChatDockLayout'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { SidecarIdsProvider } from '../contexts/sidecar-ids-context'
import { useChatKeyboardShortcuts } from '../hooks/use-chat-keyboard-shortcuts'

export function ChatPageV2() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()

  const [dockApi, setDockApi] = useState<DockviewApi | null>(null)
  const dockApiRef = useRef<DockviewApi | null>(null)

  // Sessions known to be sidecar-managed (event-driven, no polling).
  // Seeded once on mount from sidecar, then maintained via create/resume callbacks.
  // Covers the gap where sidecar owns a session but SSE control field is null.
  const [localSidecarIds, setLocalSidecarIds] = useState<Set<string>>(new Set())
  const addLocalSidecarId = useCallback((sid: string) => {
    setLocalSidecarIds((prev) => {
      if (prev.has(sid)) return prev
      const next = new Set(prev)
      next.add(sid)
      return next
    })
  }, [])

  // One-shot seed: fetch current sidecar sessions on mount (NOT polling)
  useEffect(() => {
    fetch('/api/sidecar/sessions')
      .then((r) => (r.ok ? r.json() : { active: [] }))
      .then((data: { active: { sessionId: string }[] }) => {
        if (data.active.length === 0) return
        setLocalSidecarIds((prev) => {
          const next = new Set(prev)
          for (const s of data.active) next.add(s.sessionId)
          return next
        })
      })
      .catch(() => {})
  }, [])

  useChatKeyboardShortcuts(dockApi)

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
        if (!existing.api.isActive) existing.api.setActive()
        return
      }
      // Derive isWatching: live elsewhere + NOT sidecar-managed (by SSE control OR local knowledge)
      const liveSession = liveSessions.sessions.find((s) => s.id === sid)
      const isSidecarManaged = liveSession?.control != null || localSidecarIds.has(sid)
      const isWatching = liveSession != null && !isSidecarManaged

      api.addPanel({
        id: `chat-${sid}`,
        component: 'chat',
        title: title ?? sid.slice(0, 8),
        params: { sessionId: sid, isWatching },
      })
    },
    [liveSessions.sessions, localSidecarIds],
  )

  // Open a blank "new chat" panel — returns the panel ID so callers can track it
  const openNewChat = useCallback(() => {
    const api = dockApiRef.current
    if (!api) return

    // Reuse existing blank panel if one exists
    const existing = api.panels.find((p) => (p.params as { sessionId?: string })?.sessionId === '')
    if (existing) {
      if (!existing.api.isActive) existing.api.setActive()
      return
    }

    api.addPanel({
      id: `chat-new-${Date.now()}`,
      component: 'chat',
      title: 'New Chat',
      params: { sessionId: '' },
    })
  }, [])

  // Auto-open panel when URL has sessionId, or open blank panel when at /chat
  useEffect(() => {
    if (!dockApiRef.current) return
    if (sessionId) {
      openSession(sessionId)
    } else {
      openNewChat()
    }
  }, [sessionId, openSession, openNewChat])

  const sidecarCtx = useMemo(() => ({ addLocalSidecarId }), [addLocalSidecarId])

  return (
    <SidecarIdsProvider value={sidecarCtx}>
      <div className="flex h-full overflow-hidden">
        <SessionSidebar
          liveSessions={liveSessions.sessions}
          localSidecarIds={localSidecarIds}
          onSessionCreated={addLocalSidecarId}
        />
        <div className="flex-1 flex flex-col">
          <ChatDockLayout onReady={handleDockReady} />
        </div>
      </div>
    </SidecarIdsProvider>
  )
}
