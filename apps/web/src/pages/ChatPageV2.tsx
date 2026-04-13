import { useQueryClient } from '@tanstack/react-query'
import type { DockviewApi } from 'dockview-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import { ChatDockLayout } from '../components/chat/ChatDockLayout'
import { useDockLayoutStore } from '../store/dock-layout-store'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import { useActiveSessions } from '../store/live-session-store'
import type { LiveSession } from '@claude-view/shared/types/generated'
import { useChatKeyboardShortcuts } from '../hooks/use-chat-keyboard-shortcuts'
import type { LiveContextData } from '../hooks/use-context-percent'
import type { SessionOwnership } from '@claude-view/shared/types/generated/SessionOwnership'
import type { SessionInfo } from '../types/generated/SessionInfo'

/** Derive tab title using the same logic as the sidebar's SessionListItem. */
function deriveTabTitle(
  sid: string,
  cachedSessions: SessionInfo[],
  liveSessions: LiveSession[],
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
function makeSessionPanelArgs(sid: string, cachedSessions: SessionInfo[], live: LiveSession[]) {
  const liveSession = live.find((s) => s.id === sid)
  const liveContextData: LiveContextData | undefined = liveSession
    ? {
        contextWindowTokens: liveSession.contextWindowTokens,
        statuslineContextWindowSize: liveSession.statuslineContextWindowSize ?? null,
        statuslineUsedPct: liveSession.statuslineUsedPct ?? null,
      }
    : undefined
  const ownership = liveSession?.ownership ?? null
  const tmuxSessionId = ownership?.tmux?.cliSessionId
  return {
    id: `chat-${sid}`,
    component: 'chat' as const,
    title: deriveTabTitle(sid, cachedSessions, live),
    params: {
      sessionId: sid,
      ownership,
      status: liveSession?.status ?? 'done',
      liveProjectPath: liveSession?.projectPath,
      liveContextData,
      agentStateGroup: liveSession?.agentState?.group ?? null,
      tmuxSessionId,
    },
  }
}

/** Build addPanel args for a tmux terminal tab (no Claude session yet). */
function makeTmuxPanelArgs(tmuxSessionId: string) {
  const ownership: SessionOwnership = { tmux: { cliSessionId: tmuxSessionId } }
  return {
    id: `chat-cli-${tmuxSessionId}`,
    component: 'chat' as const,
    title: `CLI: ${tmuxSessionId.slice(0, 11)}`,
    params: {
      sessionId: '',
      ownership,
      status: 'working' as const,
      tmuxSessionId,
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

export function ChatPageV2() {
  // Read layout ONCE on mount from the store snapshot — not as a reactive
  // selector. Reactive would re-render on every save, which risks dockview
  // re-initialization (handleReady dep on initialLayout). On remount
  // (navigate away + back), useState re-runs and reads the latest value.
  const [savedLayout] = useState(() => useDockLayoutStore.getState().chatLayout)
  const saveChatLayout = useDockLayoutStore((s) => s.saveChatLayout)
  const { sessionId } = useParams<{ sessionId?: string }>()
  const liveSessionsList = useActiveSessions()
  const queryClient = useQueryClient()

  const [dockApi, setDockApi] = useState<DockviewApi | null>(null)
  const dockApiRef = useRef<DockviewApi | null>(null)

  // Refs for values needed synchronously in callbacks (avoids stale closures).
  // IMPORTANT: These MUST be declared before handleDockReady, which reads them.
  // If dockview fires onReady synchronously during mount, refs would be undefined
  // if declared after the callback.
  const sessionIdRef = useRef(sessionId)
  sessionIdRef.current = sessionId
  const liveSessionsRef = useRef(liveSessionsList)
  liveSessionsRef.current = liveSessionsList
  const queryClientRef = useRef(queryClient)
  queryClientRef.current = queryClient

  useChatKeyboardShortcuts(dockApi)

  const handleDockReady = useCallback((api: DockviewApi) => {
    setDockApi(api)
    dockApiRef.current = api

    // If dockview restored from layout but URL has a specific session, ensure it's open
    const urlSessionId = sessionIdRef.current
    if (urlSessionId) {
      // All sessions (including tmux-owned) use the same chat component.
      // ChatPanel renders xterm when tmuxSessionId is in params.
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

      // All sessions use chat component — ChatPanel renders xterm when tmuxSessionId is set
      const existing = api.panels.find(
        (p) => (p.params as { sessionId?: string })?.sessionId === sid,
      )
      if (existing) {
        if (!existing.api.isActive) existing.api.setActive()
        return
      }
      const resolvedTitle =
        title ?? deriveTabTitle(sid, readSidebarCache(queryClient), liveSessionsList)
      const args = makeSessionPanelArgs(sid, readSidebarCache(queryClient), liveSessionsList)
      api.addPanel({ ...args, title: resolvedTitle })
    },
    [liveSessionsList, queryClient],
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

  // CLI (tmux) session creation — POST only; panel appears reactively via
  // useActiveSessions() → useEffect when the Spawning LiveSession arrives via SSE.
  const openNewCliSession = useCallback(async () => {
    try {
      const resp = await fetch('/api/cli-sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ args: ['--dangerously-skip-permissions'] }),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      // Panel appears reactively via useActiveSessions() → useEffect
      // when the Spawning LiveSession arrives via SSE.
    } catch (err) {
      console.error('Failed to create CLI session:', err)
    }
  }, [])

  // Handle subsequent URL navigation (e.g. clicking sidebar → /chat/:sessionId).
  // Initial mount + layout restoration is handled by dockview's fromJSON + handleDockReady.
  const lastOpenedRef = useRef<string | null>(null)
  useEffect(() => {
    if (!dockApiRef.current || !sessionId || sessionId === lastOpenedRef.current) return
    lastOpenedRef.current = sessionId
    openSession(sessionId)
  }, [sessionId, openSession])

  // Sync live data into existing tab params when SSE ticks, and reactively
  // create panels for new Spawning tmux sessions (replaces imperative addPanel
  // in openNewCliSession).
  useEffect(() => {
    const api = dockApiRef.current
    if (!api) return
    const cached = readSidebarCache(queryClient)

    // Pass 1: iterate liveSessionsList — update existing panels OR create new ones.
    for (const live of liveSessionsList) {
      const tmuxId = live.ownership?.tmux?.cliSessionId

      // Find existing panel by sessionId or tmuxSessionId
      const existing = api.panels.find((p) => {
        const params = p.params as { sessionId?: string; tmuxSessionId?: string }
        return (
          (live.id && params.sessionId === live.id) || (tmuxId && params.tmuxSessionId === tmuxId)
        )
      })

      if (existing) {
        // Update existing panel params
        const liveCtx: LiveContextData | undefined = {
          contextWindowTokens: live.contextWindowTokens,
          statuslineContextWindowSize: live.statuslineContextWindowSize ?? null,
          statuslineUsedPct: live.statuslineUsedPct ?? null,
        }
        const ownership = live.ownership ?? null
        existing.api.updateParameters({
          ownership,
          status: live.status ?? 'done',
          liveProjectPath: live.projectPath,
          liveContextData: liveCtx,
          agentStateGroup: live.agentState?.group ?? null,
          tmuxSessionId: ownership?.tmux?.cliSessionId,
        })

        // When tmux session's Claude resolves, update sessionId
        // so the panel can load conversation blocks.
        if (live.status !== 'spawning' && live.id && tmuxId) {
          const curSid = (existing.params as { sessionId?: string })?.sessionId
          if (!curSid || curSid === '') {
            existing.api.updateParameters({ sessionId: live.id })
          }
        }

        const sid = live.id || tmuxId || ''
        const title = deriveTabTitle(sid, cached, liveSessionsList)
        if (title && title !== existing.title) {
          existing.api.setTitle(title)
        }
      } else if (tmuxId) {
        // Reactive panel creation: auto-create panel for any tmux session without a panel.
        // Covers Spawning (just POST'd) and Working (Born already fired, or page refreshed).
        const args = makeTmuxPanelArgs(tmuxId)
        api.addPanel(args)
        const added = api.panels.find((p) => p.id === args.id)
        if (added && !added.api.isActive) added.api.setActive()
      }
    }

    // Pass 2: update titles for panels with sessions not in liveSessionsList
    // (historical panels that need title refresh from sidebar cache).
    for (const panel of api.panels) {
      const sid = (panel.params as { sessionId?: string })?.sessionId
      if (!sid) continue
      // Skip if already handled in pass 1
      const handledByLive = liveSessionsList.some(
        (s) =>
          s.id === sid ||
          s.ownership?.tmux?.cliSessionId ===
            (panel.params as { tmuxSessionId?: string })?.tmuxSessionId,
      )
      if (handledByLive) continue

      const title = deriveTabTitle(sid, cached, liveSessionsList)
      if (title !== sid.slice(0, 8) && panel.title === sid.slice(0, 8)) {
        panel.api.setTitle(title)
      }
    }
  }, [liveSessionsList, queryClient])

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar
        liveSessions={liveSessionsList}
        onNewChat={openNewChat}
        onNewCliSession={openNewCliSession}
      />
      <div className="flex-1 min-w-0 flex flex-col overflow-hidden">
        <ChatDockLayout
          initialLayout={savedLayout}
          onReady={handleDockReady}
          onLayoutChange={saveChatLayout}
        />
      </div>
    </div>
  )
}
