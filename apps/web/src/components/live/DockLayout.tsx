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
import type { DisplayMode } from '../../store/monitor-store'
import { useMonitorStore } from '../../store/monitor-store'
import { CliTerminal } from '../cli-terminal/CliTerminal'
import { CliTerminalPanel } from '../cli-terminal/CliTerminalPanel'
import { CliTerminalTabRenderer } from '../cli-terminal/CliTerminalTabRenderer'
import { TabContent } from '../dockview/TabContent'
import { TabContextMenu } from '../dockview/TabContextMenu'
import { BlockTerminalPane } from './BlockTerminalPane'
import { MonitorPane } from './MonitorPane'
import type { LiveSession } from './use-live-sessions'

// --- Context: provides session data + callbacks to dockview panel components ---

interface DockPaneContextValue {
  sessions: LiveSession[]
  onExpandSession?: (id: string) => void
}

const DockPaneContext = createContext<DockPaneContextValue>({ sessions: [] })

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
}: IDockviewPanelProps<{
  sessionId: string
  displayMode?: DisplayMode
  status: string
}>) {
  const sessionId = params.sessionId
  const { sessions, onExpandSession } = useContext(DockPaneContext)
  const session = sessions.find((s) => s.id === sessionId)

  // Store state + actions — read directly so panels stay in sync
  const compactHeaders = useMonitorStore((s) => s.compactHeaders)
  const selectedPaneId = useMonitorStore((s) => s.selectedPaneId)
  const pinnedPaneIds = useMonitorStore((s) => s.pinnedPaneIds)
  const selectPane = useMonitorStore((s) => s.selectPane)
  const pinPane = useMonitorStore((s) => s.pinPane)
  const unpinPane = useMonitorStore((s) => s.unpinPane)
  const hidePane = useMonitorStore((s) => s.hidePane)

  if (!session) {
    return (
      <div className="flex-1 bg-white dark:bg-[#0D1117] p-4 text-gray-500 dark:text-[#8B949E]">
        Session ended
      </div>
    )
  }

  const isPinned = pinnedPaneIds.has(sessionId)

  return (
    <MonitorPane
      session={session}
      isSelected={selectedPaneId === sessionId}
      isExpanded={false}
      isPinned={isPinned}
      compactHeader={compactHeaders}
      isVisible={true}
      embedded
      onSelect={() => selectPane(selectedPaneId === sessionId ? null : sessionId)}
      onExpand={() => onExpandSession?.(sessionId)}
      onPin={() => (isPinned ? unpinPane(sessionId) : pinPane(sessionId))}
      onHide={() => hidePane(sessionId)}
      onContextMenu={() => {}}
    >
      {session.ownership?.tmux ? (
        <CliTerminal tmuxSessionId={session.ownership.tmux.cliSessionId} className="h-full" />
      ) : (
        <BlockTerminalPane sessionId={sessionId} isVisible={true} />
      )}
    </MonitorPane>
  )
}

// Component registry + watermark — defined outside the component to avoid
// re-creating on every render (React reconciler uses referential equality).
const components = { session: SessionPanel, cliTerminal: CliTerminalPanel }

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
  const sessionsRef = useRef(sessions)
  sessionsRef.current = sessions
  const displayModeRef = useRef(displayMode)
  displayModeRef.current = displayMode
  const onLayoutChangeRef = useRef(onLayoutChange)
  onLayoutChangeRef.current = onLayoutChange

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
          // Restore saved layout
          event.api.fromJSON(initialLayout)
          restored = true
          // Update panel params with current displayMode
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
            params: {
              sessionId: id,
              displayMode: currentDisplayMode,
              status: session?.status ?? 'done',
              agentStateGroup: session?.agentState?.group ?? null,
              tmuxSessionId: session?.ownership?.tmux?.cliSessionId,
            },
            // First panel gets its own group, rest stack or split
            position: i === 0 ? undefined : { referencePanel: ids[0], direction: 'right' },
          })
        }
      }

      // Listen for structural layout changes (add/remove/move panels, resize)
      // and persist. Debounce avoids N localStorage.setItem calls during bulk
      // mutations (e.g. preset application that adds 4 panels in quick succession).
      let debounceTimer: ReturnType<typeof setTimeout> | null = null
      const persistLayout = () => {
        if (debounceTimer) clearTimeout(debounceTimer)
        debounceTimer = setTimeout(() => {
          if (apiRef.current) {
            onLayoutChangeRef.current(apiRef.current.toJSON())
          }
        }, 100)
      }
      event.api.onDidAddPanel(persistLayout)
      event.api.onDidRemovePanel(persistLayout)
      event.api.onDidLayoutChange(persistLayout)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refs are stable; initialLayout is the only true dep
    [initialLayout, onApiReady],
  )

  // Sync session data into existing panels when sessions update.
  // CLI terminal panels (tmuxSessionId in params) have their own lifecycle
  // and are NOT tied to the live sessions list — skip them entirely.
  useEffect(() => {
    const api = apiRef.current
    if (!api) return

    // Standalone CLI panels (from handleCliSessionCreated) have tmuxSessionId
    // but NO sessionId — they're not tied to the live sessions list.
    // Session panels for tmux-owned sessions have BOTH sessionId + tmuxSessionId.
    const isStandaloneCliPanel = (p: { params: unknown }) => {
      const pp = p.params as { sessionId?: string; tmuxSessionId?: string } | undefined
      return !!pp?.tmuxSessionId && !pp?.sessionId
    }

    // Update existing session panels with fresh displayMode + title
    for (const panel of api.panels) {
      if (isStandaloneCliPanel(panel)) continue
      const session = sessions.find((s) => s.id === panel.id)
      if (session) {
        panel.api.updateParameters({
          sessionId: session.id,
          displayMode,
          status: session.status,
          agentStateGroup: session.agentState?.group ?? null,
          tmuxSessionId: session.ownership?.tmux?.cliSessionId,
        })
        // Align title with ChatPageV2: slug > projectDisplayName > id
        const title = session.slug || session.projectDisplayName || session.id.slice(0, 8)
        if (title !== panel.title) {
          panel.api.setTitle(title)
        }
      }
    }

    // Add panels for new sessions.
    // If CLI terminal panels exist, add session panels as inactive so they
    // don't steal focus from the xterm terminal (tmux has higher precedence).
    const existingIds = new Set(api.panels.map((p) => p.id))
    const hasCliPanels = api.panels.some((p) => isStandaloneCliPanel(p))
    for (const session of sessions) {
      if (!existingIds.has(session.id)) {
        api.addPanel({
          id: session.id,
          component: 'session',
          title: session.slug || session.projectDisplayName || session.id.slice(0, 8),
          params: {
            sessionId: session.id,
            displayMode,
            status: session.status,
            agentStateGroup: session.agentState?.group ?? null,
            tmuxSessionId: session.ownership?.tmux?.cliSessionId,
          },
          inactive: hasCliPanels,
        })
      }
    }

    // Remove panels for ended sessions.
    // IMPORTANT: Snapshot the array first — calling removePanel() mutates
    // api.panels in place, which causes iterator invalidation if we iterate
    // the live array directly (same pattern as Array.prototype.filter-then-forEach).
    const currentIds = new Set(sessions.map((s) => s.id))
    const panelsToRemove = api.panels.filter(
      (p) => !isStandaloneCliPanel(p) && !currentIds.has(p.id),
    )
    for (const panel of panelsToRemove) {
      api.removePanel(panel)
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
          tabComponents={{ session: SessionTabRenderer, cliTerminal: CliTerminalTabRenderer }}
          defaultTabComponent={SessionTabRenderer}
          onReady={onReady}
          watermarkComponent={EmptyWatermark}
        />
      </div>
    </DockPaneContext.Provider>
  )
}
