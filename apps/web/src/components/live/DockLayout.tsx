import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  type IDockviewPanelHeaderProps,
  type IDockviewPanelProps,
  type IWatermarkPanelProps,
  type SerializedDockview,
} from 'dockview-react'

// Custom dockview theme — must be passed via `theme` prop to prevent dockview
// from defaulting to themeAbyss (which adds .dockview-theme-abyss and overrides
// our light-mode CSS variables in dockview-theme.css).
const cvTheme = { name: 'cv', className: 'dockview-theme-cv' }
import { createContext, useCallback, useContext, useEffect, useRef } from 'react'
import { useDockviewPersistence } from '../../hooks/use-dockview-persistence'
import { MIN_PANE_H, MIN_PANE_W } from '../../hooks/use-auto-layout'
import { useFocusFollowsMouse } from '../../hooks/use-focus-follows-mouse'
import type { DisplayMode } from '../../store/monitor-store'
import { CliTerminal } from '../cli-terminal/CliTerminal'
import { TabContent } from '../dockview/TabContent'
import { TabContextMenu } from '../dockview/TabContextMenu'
import { BlockTerminalPane } from './BlockTerminalPane'
import type { LiveSession } from './use-live-sessions'

// --- Context: provides session data + callbacks to dockview panel components ---

export interface DockPaneContextValue {
  sessions: LiveSession[]
  onExpandSession?: (id: string) => void
}

export const DockPaneContext = createContext<DockPaneContextValue>({ sessions: [] })

// --- Props ---

interface DockLayoutProps {
  sessions: LiveSession[]
  /** Restore from this layout on mount (from localStorage or preset). */
  initialLayout: SerializedDockview | null
  /** Called whenever the layout changes structurally (resize, move, tab reorder). */
  onLayoutChange: (layout: SerializedDockview) => void
  /** Called once when the dockview API is ready — use to capture the API ref in the parent. */
  onApiReady?: (api: DockviewApi) => void
  compactHeaders: boolean
  displayMode: DisplayMode
  onSelectSession?: (id: string) => void
}

// --- Panel component rendered inside each dockview panel ---

function SessionPanel({
  params,
  containerApi,
}: IDockviewPanelProps<{
  sessionId: string
  displayMode?: DisplayMode
  status: string
}>) {
  const sessionId = params.sessionId
  const { sessions } = useContext(DockPaneContext)
  const session = sessions.find((s) => s.id === sessionId)
  const { getHandlers } = useFocusFollowsMouse({ enabled: true })

  const focusHandlers = getHandlers(() => {
    // containerApi.getPanel() returns the full panel object with focus()
    containerApi.getPanel(sessionId)?.focus()
  })

  if (!session) {
    return (
      <div className="flex-1 bg-white dark:bg-[#0D1117] p-4 text-gray-500 dark:text-[#8B949E]">
        Session ended
      </div>
    )
  }

  // Dockview tab bar IS the chrome — render terminal directly, no MonitorPane wrapper.
  // MonitorPane is still used in auto-grid mode (MonitorGrid) but never inside dockview panels.
  return (
    <div
      className="h-full"
      onPointerEnter={focusHandlers.onPointerEnter}
      onPointerLeave={focusHandlers.onPointerLeave}
    >
      {session.ownership?.tmux ? (
        <CliTerminal
          tmuxSessionId={session.ownership.tmux.cliSessionId}
          className="h-full"
          embedded
        />
      ) : (
        <BlockTerminalPane sessionId={sessionId} isVisible={true} />
      )}
    </div>
  )
}

// Component registry + watermark — defined outside the component to avoid
// re-creating on every render (React reconciler uses referential equality).
const components = { session: SessionPanel }

function EmptyWatermark(_props: IWatermarkPanelProps) {
  return (
    <div className="flex items-center justify-center h-full text-gray-400 dark:text-[#8B949E] text-sm">
      No sessions. Start a Claude Code session to see it here.
    </div>
  )
}

function SessionTabRenderer({ api, params, containerApi }: IDockviewPanelHeaderProps) {
  const status = (params.status as string | null) ?? null
  const agentStateGroup = (params.agentStateGroup as string | null) ?? null
  const tmuxSessionId = (params.tmuxSessionId as string | undefined) ?? undefined
  const isTmux = !!tmuxSessionId

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (tmuxSessionId) {
      fetch(`/api/cli-sessions/${tmuxSessionId}`, { method: 'DELETE' }).catch(() => {})
    }
    api.close()
  }

  const handleMiddleClick = (e: React.MouseEvent) => {
    if (e.button === 1) handleClose(e)
  }

  const panel = containerApi.panels.find((p) => p.id === api.id)

  const tab = (
    <TabContent
      title={api.title ?? ''}
      status={status}
      agentStateGroup={agentStateGroup}
      isTmux={isTmux}
      onClose={handleClose}
      onMiddleClick={handleMiddleClick}
    />
  )

  if (panel) {
    return (
      <TabContextMenu panel={panel} api={containerApi} splitComponent="session">
        {tab}
      </TabContextMenu>
    )
  }

  return tab
}

export function DockLayout({
  sessions,
  initialLayout,
  onLayoutChange,
  onApiReady,
  compactHeaders: _compactHeaders,
  displayMode,
  onSelectSession,
}: DockLayoutProps) {
  const apiRef = useRef<DockviewApi | null>(null)
  // Track session IDs that have been seen — only auto-add panels for genuinely
  // NEW sessions (first appearance). Without this, the useEffect re-adds panels
  // for sessions the user manually closed, because the session is still live.
  const knownSessionIdsRef = useRef(new Set(sessions.map((s) => s.id)))
  const sessionsRef = useRef(sessions)
  sessionsRef.current = sessions
  const displayModeRef = useRef(displayMode)
  displayModeRef.current = displayMode

  const attachListeners = useDockviewPersistence(onLayoutChange)

  // onReady fires ONCE when dockview mounts. All mutable values (sessions,
  // displayMode) are read via refs so the callback identity is stable and
  // dockview never re-initializes on SSE ticks.
  const onReady = useCallback(
    (event: DockviewReadyEvent) => {
      apiRef.current = event.api
      onApiReady?.(event.api)

      const currentSessions = sessionsRef.current
      const currentDisplayMode = displayModeRef.current

      let restored = false
      if (initialLayout) {
        try {
          event.api.fromJSON(initialLayout)
          restored = true
          // Remove panels for sessions no longer active
          const currentIds = new Set(currentSessions.map((s) => s.id))
          const stalePanels = event.api.panels.filter((p) => !currentIds.has(p.id))
          for (const p of stalePanels) event.api.removePanel(p)

          // Update surviving panels with fresh session data
          for (const panel of event.api.panels) {
            const session = currentSessions.find((s) => s.id === panel.id)
            if (session) {
              panel.api.updateParameters({
                sessionId: session.id,
                displayMode: currentDisplayMode,
                status: session.status,
                agentStateGroup: session.agentState?.group ?? null,
                tmuxSessionId: session.ownership?.tmux?.cliSessionId,
              })
            }
          }

          // Add panels for new sessions not in saved layout
          const restoredIds = new Set(event.api.panels.map((p) => p.id))
          for (const session of currentSessions) {
            if (!restoredIds.has(session.id)) {
              event.api.addPanel({
                id: session.id,
                component: 'session',
                title: session.slug || session.projectDisplayName || session.id.slice(0, 8),
                minimumWidth: MIN_PANE_W,
                minimumHeight: MIN_PANE_H,
                params: {
                  sessionId: session.id,
                  displayMode: currentDisplayMode,
                  status: session.status,
                  agentStateGroup: session.agentState?.group ?? null,
                  tmuxSessionId: session.ownership?.tmux?.cliSessionId,
                },
              })
            }
          }
        } catch {
          // Corrupt or incompatible layout — fall through to auto-build
          event.api.clear()
        }
      }
      if (!restored) {
        // Build initial layout from current sessions
        const ids = currentSessions.map((s) => s.id)
        for (const [i, id] of ids.entries()) {
          const session = currentSessions.find((s) => s.id === id)
          event.api.addPanel({
            id,
            component: 'session',
            title: session?.slug || session?.projectDisplayName || id.slice(0, 8),
            minimumWidth: MIN_PANE_W,
            minimumHeight: MIN_PANE_H,
            params: {
              sessionId: id,
              displayMode: currentDisplayMode,
              status: session?.status ?? 'done',
              agentStateGroup: session?.agentState?.group ?? null,
              tmuxSessionId: session?.ownership?.tmux?.cliSessionId,
            },
            position: i === 0 ? undefined : { referencePanel: ids[0], direction: 'right' },
          })
        }
      }

      // Mark all current sessions as known so useEffect doesn't re-add them
      for (const s of currentSessions) knownSessionIdsRef.current.add(s.id)

      attachListeners(event.api)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refs are stable; initialLayout is the only true dep
    [initialLayout, onApiReady, attachListeners],
  )

  useEffect(() => {
    const api = apiRef.current
    if (!api) return

    // Update existing session panels with fresh data
    for (const panel of api.panels) {
      const session = sessions.find((s) => s.id === panel.id)
      if (session) {
        panel.api.updateParameters({
          sessionId: session.id,
          displayMode,
          status: session.status,
          agentStateGroup: session.agentState?.group ?? null,
          tmuxSessionId: session.ownership?.tmux?.cliSessionId,
        })
        const title = session.slug || session.projectDisplayName || session.id.slice(0, 8)
        if (title !== panel.title) {
          panel.api.setTitle(title)
        }
      }
    }

    // Add panels only for genuinely NEW sessions (not seen before).
    // Without knownSessionIdsRef, every SSE tick would re-add panels the user
    // manually closed — the session is still live, so it looks "missing."
    const existingIds = new Set(api.panels.map((p) => p.id))
    for (const session of sessions) {
      if (!existingIds.has(session.id) && !knownSessionIdsRef.current.has(session.id)) {
        api.addPanel({
          id: session.id,
          component: 'session',
          title: session.slug || session.projectDisplayName || session.id.slice(0, 8),
          minimumWidth: MIN_PANE_W,
          minimumHeight: MIN_PANE_H,
          params: {
            sessionId: session.id,
            displayMode,
            status: session.status,
            agentStateGroup: session.agentState?.group ?? null,
            tmuxSessionId: session.ownership?.tmux?.cliSessionId,
          },
        })
      }
      knownSessionIdsRef.current.add(session.id)
    }

    // Remove panels for ended sessions.
    // Snapshot the array first — removePanel() mutates api.panels in place.
    const currentIds = new Set(sessions.map((s) => s.id))
    const panelsToRemove = api.panels.filter((p) => !currentIds.has(p.id))
    for (const panel of panelsToRemove) {
      api.removePanel(panel)
    }
    // Prune ended sessions from known set so they get auto-added if they return
    for (const id of knownSessionIdsRef.current) {
      if (!currentIds.has(id)) knownSessionIdsRef.current.delete(id)
    }
  }, [sessions, displayMode])

  // Context value — memoized via ref to avoid unnecessary re-renders.
  // Sessions array changes on every SSE tick but dockview panels read from
  // context only on render (triggered by param updates above).
  const contextValue: DockPaneContextValue = {
    sessions,
    onExpandSession: onSelectSession,
  }

  return (
    <DockPaneContext.Provider value={contextValue}>
      <div className="absolute inset-0">
        <DockviewReact
          theme={cvTheme}
          components={components}
          tabComponents={{ session: SessionTabRenderer }}
          defaultTabComponent={SessionTabRenderer}
          onReady={onReady}
          watermarkComponent={EmptyWatermark}
        />
      </div>
    </DockPaneContext.Provider>
  )
}
