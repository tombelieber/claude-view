import { useQueryClient } from '@tanstack/react-query'
import type { DockviewApi, SerializedDockview } from 'dockview-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useOutletContext, useParams } from 'react-router-dom'
import { ChatDockLayout, readSavedChatLayout } from '../components/chat/ChatDockLayout'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import type { UseLiveSessionsResult } from '../components/live/use-live-sessions'
import { useChatKeyboardShortcuts } from '../hooks/use-chat-keyboard-shortcuts'
import type { LiveContextData } from '../hooks/use-context-percent'
import { deriveLiveStatus } from '../lib/live-status'
import type { SessionInfo } from '../types/generated/SessionInfo'

/** Derive tab title using the same logic as the sidebar's SessionListItem. */
function deriveTabTitle(
  sid: string,
  cachedSessions: SessionInfo[],
  liveSessions: UseLiveSessionsResult['sessions'],
): string {
  const cached = cachedSessions.find((s) => s.id === sid)
  if (cached) {
    return cached.slug || cached.preview?.slice(0, 60) || sid.slice(0, 8)
  }
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

/** Build addPanel args for a session tab. Single source of truth for panel shape. */
function makeSessionPanelArgs(
  sid: string,
  cachedSessions: SessionInfo[],
  live: UseLiveSessionsResult['sessions'],
) {
  const liveSession = live.find((s) => s.id === sid)
  const liveContextData: LiveContextData | undefined = liveSession
    ? {
        contextWindowTokens: liveSession.contextWindowTokens,
        statuslineContextWindowSize: liveSession.statuslineContextWindowSize ?? null,
        statuslineUsedPct: liveSession.statuslineUsedPct ?? null,
      }
    : undefined
  return {
    id: `chat-${sid}`,
    component: 'chat' as const,
    title: deriveTabTitle(sid, cachedSessions, live),
    params: {
      sessionId: sid,
      liveStatus: deriveLiveStatus(liveSession),
      liveProjectPath: liveSession?.projectPath,
      liveContextData,
      agentStateGroup: liveSession?.agentState?.group ?? null,
    },
  }
}

/** Build addPanel args for a blank "New Chat" tab. */
function makeNewChatPanelArgs() {
  return {
    id: `chat-new-${Date.now()}`,
    component: 'chat' as const,
    title: 'New Chat',
    params: { sessionId: '' },
  }
}

// Read saved layout once at module level — stable across renders.
// Dockview's fromJSON restores panels, groups, sizes, and active tab natively.
const savedLayout: SerializedDockview | null = readSavedChatLayout()

export function ChatPageV2() {
  const { sessionId } = useParams<{ sessionId?: string }>()
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()
  const queryClient = useQueryClient()

  const [dockApi, setDockApi] = useState<DockviewApi | null>(null)
  const dockApiRef = useRef<DockviewApi | null>(null)

  // Refs for values needed synchronously in callbacks (avoids stale closures).
  // IMPORTANT: These MUST be declared before handleDockReady, which reads them.
  // If dockview fires onReady synchronously during mount, refs would be undefined
  // if declared after the callback.
  const sessionIdRef = useRef(sessionId)
  sessionIdRef.current = sessionId
  const liveSessionsRef = useRef(liveSessions.sessions)
  liveSessionsRef.current = liveSessions.sessions
  const queryClientRef = useRef(queryClient)
  queryClientRef.current = queryClient

  useChatKeyboardShortcuts(dockApi)

  const handleDockReady = useCallback((api: DockviewApi) => {
    setDockApi(api)
    dockApiRef.current = api

    // If dockview restored from layout but URL has a specific session, ensure it's open
    const urlSessionId = sessionIdRef.current
    if (urlSessionId) {
      const exists = api.panels.find(
        (p) => (p.params as { sessionId?: string })?.sessionId === urlSessionId,
      )
      if (exists) {
        if (!exists.api.isActive) exists.api.setActive()
      } else {
        const cached = readSidebarCache(queryClientRef.current)
        const live = liveSessionsRef.current
        api.addPanel(makeSessionPanelArgs(urlSessionId, cached, live))
        const added = api.panels.find(
          (p) => (p.params as { sessionId?: string })?.sessionId === urlSessionId,
        )
        if (added && !added.api.isActive) added.api.setActive()
      }
    } else if (api.panels.length === 0) {
      // No saved layout and no URL session → blank panel
      api.addPanel(makeNewChatPanelArgs())
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- all mutable values via refs; identity must be stable for dockview
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
      const resolvedTitle =
        title ?? deriveTabTitle(sid, readSidebarCache(queryClient), liveSessions.sessions)
      const args = makeSessionPanelArgs(sid, readSidebarCache(queryClient), liveSessions.sessions)
      api.addPanel({ ...args, title: resolvedTitle })
    },
    [liveSessions.sessions, queryClient],
  )

  const openNewChat = useCallback(() => {
    const api = dockApiRef.current
    if (!api) return
    const args = makeNewChatPanelArgs()
    api.addPanel(args)
    // Look up by ID — dockview doesn't guarantee insertion order in api.panels
    const added = api.panels.find((p) => p.id === args.id)
    if (added && !added.api.isActive) added.api.setActive()
  }, [])

  // Handle subsequent URL navigation (e.g. clicking sidebar → /chat/:sessionId).
  // Initial mount + layout restoration is handled by dockview's fromJSON + handleDockReady.
  const lastOpenedRef = useRef<string | null>(null)
  useEffect(() => {
    if (!dockApiRef.current || !sessionId || sessionId === lastOpenedRef.current) return
    lastOpenedRef.current = sessionId
    openSession(sessionId)
  }, [sessionId, openSession])

  // Sync live data (dot color, title, liveStatus) into existing tab params when SSE ticks.
  // This corrects any stale values from layout-restore time (SSE hadn't delivered data yet).
  useEffect(() => {
    const api = dockApiRef.current
    if (!api) return
    const cached = readSidebarCache(queryClient)

    for (const panel of api.panels) {
      const sid = (panel.params as { sessionId?: string })?.sessionId
      if (!sid) continue
      const live = liveSessions.sessions.find((s) => s.id === sid)
      const liveCtx: LiveContextData | undefined = live
        ? {
            contextWindowTokens: live.contextWindowTokens,
            statuslineContextWindowSize: live.statuslineContextWindowSize ?? null,
            statuslineUsedPct: live.statuslineUsedPct ?? null,
          }
        : undefined
      panel.api.updateParameters({
        liveStatus: deriveLiveStatus(live),
        liveProjectPath: live?.projectPath,
        liveContextData: liveCtx,
        agentStateGroup: live?.agentState?.group ?? null,
      })
      const title = deriveTabTitle(sid, cached, liveSessions.sessions)
      if (title !== sid.slice(0, 8) && panel.title === sid.slice(0, 8)) {
        panel.api.setTitle(title)
      }
    }
  }, [liveSessions.sessions, queryClient])

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar liveSessions={liveSessions.sessions} onNewChat={openNewChat} />
      <div className="flex-1 min-w-0 flex flex-col overflow-hidden">
        <ChatDockLayout initialLayout={savedLayout} onReady={handleDockReady} />
      </div>
    </div>
  )
}
