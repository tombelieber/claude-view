import { useQueryClient } from '@tanstack/react-query'
import type { DockviewApi, SerializedDockview } from 'dockview-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useOutletContext, useParams } from 'react-router-dom'
import { useCliSessions, useCreateCliSession } from '../hooks/use-cli-sessions'
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

  // CLI session data — used to detect tmux-owned sessions and open them as xterm.
  const { data: cliSessions } = useCliSessions()
  const cliSessionsRef = useRef(cliSessions)
  cliSessionsRef.current = cliSessions

  useChatKeyboardShortcuts(dockApi)

  const handleDockReady = useCallback((api: DockviewApi) => {
    setDockApi(api)
    dockApiRef.current = api

    // If dockview restored from layout but URL has a specific session, ensure it's open
    const urlSessionId = sessionIdRef.current
    if (urlSessionId) {
      // Check if this is a CLI (tmux) session — open as xterm instead
      const cliMatch = cliSessionsRef.current?.find((c) => c.claudeSessionId === urlSessionId)
      if (cliMatch) {
        const panelId = `cli-${cliMatch.id}`
        const existingCli = api.panels.find((p) => p.id === panelId)
        if (existingCli) {
          if (!existingCli.api.isActive) existingCli.api.setActive()
        } else {
          api.addPanel({
            id: panelId,
            component: 'cliTerminal',
            tabComponent: 'cliTerminal',
            title: `CLI: ${cliMatch.id.slice(0, 11)}`,
            params: { tmuxSessionId: cliMatch.id },
          })
        }
      } else {
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

      // Check if this session belongs to a CLI (tmux) session → open as xterm
      const cliMatch = cliSessionsRef.current?.find((c) => c.claudeSessionId === sid)
      if (cliMatch) {
        const panelId = `cli-${cliMatch.id}`
        const existingCli = api.panels.find((p) => p.id === panelId)
        if (existingCli) {
          if (!existingCli.api.isActive) existingCli.api.setActive()
          return
        }
        api.addPanel({
          id: panelId,
          component: 'cliTerminal',
          tabComponent: 'cliTerminal',
          title: `CLI: ${cliMatch.id.slice(0, 11)}`,
          params: { tmuxSessionId: cliMatch.id },
        })
        const added = api.panels.find((p) => p.id === panelId)
        if (added && !added.api.isActive) added.api.setActive()
        return
      }

      // Regular session — open as ConversationView
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

  // CLI (tmux) session creation — opens an xterm panel in the chat dock.
  const createCliSession = useCreateCliSession()
  const openNewCliSession = useCallback(async () => {
    const api = dockApiRef.current
    if (!api) return
    try {
      const { session } = await createCliSession.mutateAsync({
        args: ['--dangerously-skip-permissions'],
      })
      const panelId = `cli-${session.id}`
      api.addPanel({
        id: panelId,
        component: 'cliTerminal',
        tabComponent: 'cliTerminal',
        title: `CLI: ${session.id.slice(0, 11)}`,
        params: { tmuxSessionId: session.id },
      })
      const added = api.panels.find((p) => p.id === panelId)
      if (added && !added.api.isActive) added.api.setActive()
    } catch {
      // useCreateCliSession already handles error via mutation state
    }
  }, [createCliSession])

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
  // CLI terminal panels (id starts with "cli-") have their own lifecycle — skip them.
  useEffect(() => {
    const api = dockApiRef.current
    if (!api) return
    const cached = readSidebarCache(queryClient)

    for (const panel of api.panels) {
      if (panel.id.startsWith('cli-')) continue
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

  // Replace chat panels with xterm panels when CLI session data arrives.
  // Handles the race where dockview restores/opens a panel before useCliSessions resolves.
  useEffect(() => {
    const api = dockApiRef.current
    if (!api || !cliSessions?.length) return

    // Build reverse map: claudeSessionId → cliSession
    const claudeToTmux = new Map<string, { id: string }>(
      cliSessions
        .filter((c) => c.claudeSessionId && c.status === 'running')
        .map((c) => [c.claudeSessionId!, { id: c.id }]),
    )
    if (claudeToTmux.size === 0) return

    for (const panel of [...api.panels]) {
      if (panel.id.startsWith('cli-')) continue
      const sid = (panel.params as { sessionId?: string })?.sessionId
      if (!sid) continue
      const cli = claudeToTmux.get(sid)
      if (!cli) continue

      // This chat panel belongs to a CLI session — replace with xterm
      const panelId = `cli-${cli.id}`
      if (api.panels.some((p) => p.id === panelId)) {
        // xterm panel already exists, just remove the chat one
        api.removePanel(panel)
        continue
      }
      const wasActive = panel.api.isActive
      api.removePanel(panel)
      api.addPanel({
        id: panelId,
        component: 'cliTerminal',
        tabComponent: 'cliTerminal',
        title: `CLI: ${cli.id.slice(0, 11)}`,
        params: { tmuxSessionId: cli.id },
        inactive: !wasActive,
      })
    }
  }, [cliSessions])

  return (
    <div className="flex h-full overflow-hidden">
      <SessionSidebar
        liveSessions={liveSessions.sessions}
        onNewChat={openNewChat}
        onNewCliSession={openNewCliSession}
      />
      <div className="flex-1 min-w-0 flex flex-col overflow-hidden">
        <ChatDockLayout initialLayout={savedLayout} onReady={handleDockReady} />
      </div>
    </div>
  )
}
