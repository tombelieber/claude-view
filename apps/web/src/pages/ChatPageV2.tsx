import { useQueryClient } from '@tanstack/react-query'
import type { DockviewApi } from 'dockview-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useOutletContext, useParams } from 'react-router-dom'
import { ChatDockLayout } from '../components/chat/ChatDockLayout'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { useChatKeyboardShortcuts } from '../hooks/use-chat-keyboard-shortcuts'
import { readPersistedTabs, useChatTabPersistence } from '../hooks/use-chat-tab-persistence'
import type { SessionInfo } from '../types/generated/SessionInfo'

/** Derive tab title using the same logic as the sidebar's SessionListItem. */
function deriveTabTitle(
  sid: string,
  cachedSessions: SessionInfo[],
  liveSessions: UseLiveSessionsResult['sessions'],
): string {
  // Check sidebar cache first (has slug + preview from history)
  const cached = cachedSessions.find((s) => s.id === sid)
  if (cached) {
    return cached.slug || cached.preview?.slice(0, 60) || sid.slice(0, 8)
  }
  // Fall back to live session data (newly created, not yet indexed)
  const live = liveSessions.find((s) => s.id === sid)
  if (live) {
    return live.slug || live.projectDisplayName || sid.slice(0, 8)
  }
  return sid.slice(0, 8)
}

/** Read all SessionInfo from the sidebar's React Query cache. */
function readSidebarCache(queryClient: ReturnType<typeof useQueryClient>): SessionInfo[] {
  const data = queryClient.getQueryData<{
    pages: { sessions: SessionInfo[] }[]
  }>(['chat-sidebar-sessions'])
  return data?.pages.flatMap((p) => p.sessions) ?? []
}

export function ChatPageV2() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()
  const queryClient = useQueryClient()

  const [dockApi, setDockApi] = useState<DockviewApi | null>(null)
  const dockApiRef = useRef<DockviewApi | null>(null)

  useChatKeyboardShortcuts(dockApi)
  useChatTabPersistence(dockApi)
  const restoredRef = useRef(false)

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
      // Derive isWatching from SSE LiveSession.control field (set by Rust server
      // when sidecar notifies via POST /api/live/sessions/:id/bind-control).
      // No local tracking needed — the sidecar→Rust server→SSE pipeline is the single source.
      const liveSession = liveSessions.sessions.find((s) => s.id === sid)
      const isWatching = liveSession != null && liveSession.control == null

      const resolvedTitle =
        title ?? deriveTabTitle(sid, readSidebarCache(queryClient), liveSessions.sessions)

      api.addPanel({
        id: `chat-${sid}`,
        component: 'chat',
        title: resolvedTitle,
        params: {
          sessionId: sid,
          isWatching,
          agentStateGroup: liveSession?.agentState?.group ?? null,
          hasLiveData: liveSession != null,
          isSidecarManaged: liveSession?.control != null,
        },
      })
    },
    [liveSessions.sessions, queryClient],
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

  // Restore persisted tabs on first dock ready, then handle URL-driven opens.
  // Order: restore saved tabs → open URL session (may already be in saved set).
  useEffect(() => {
    if (!dockApiRef.current) return

    // One-time restore from localStorage on mount
    if (!restoredRef.current) {
      restoredRef.current = true
      const { openTabs, activeTab } = readPersistedTabs()
      for (const sid of openTabs) {
        if (sid === '') {
          openNewChat()
        } else {
          openSession(sid)
        }
      }
      // Re-activate the previously focused tab (openSession already activates
      // if it's the last one, but we need to restore the exact active tab)
      if (activeTab && dockApiRef.current) {
        const target = dockApiRef.current.panels.find(
          (p) => (p.params as { sessionId?: string })?.sessionId === activeTab,
        )
        if (target && !target.api.isActive) target.api.setActive()
      }
    }

    // URL-driven open: sessionId from route param
    if (sessionId) {
      openSession(sessionId)
    } else if (dockApiRef.current.panels.length === 0) {
      // No persisted tabs and no sessionId → blank panel
      openNewChat()
    }
  }, [sessionId, openSession, openNewChat])

  // Sync live data (dot color, title) into existing tab params when SSE ticks.
  // This keeps tabs aligned with the sidebar as sessions change state.
  useEffect(() => {
    const api = dockApiRef.current
    if (!api) return
    const cached = readSidebarCache(queryClient)

    for (const panel of api.panels) {
      const sid = (panel.params as { sessionId?: string })?.sessionId
      if (!sid) continue
      const live = liveSessions.sessions.find((s) => s.id === sid)
      panel.api.updateParameters({
        agentStateGroup: live?.agentState?.group ?? null,
        hasLiveData: live != null,
        isSidecarManaged: live?.control != null,
      })
      // Also update title if it was previously just a truncated ID
      const title = deriveTabTitle(sid, cached, liveSessions.sessions)
      if (title !== sid.slice(0, 8) && panel.title === sid.slice(0, 8)) {
        panel.api.setTitle(title)
      }
    }
  }, [liveSessions.sessions, queryClient])

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar liveSessions={liveSessions.sessions} />
      <div className="flex-1 flex flex-col">
        <ChatDockLayout onReady={handleDockReady} />
      </div>
    </div>
  )
}
