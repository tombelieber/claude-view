import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  type IDockviewPanelHeaderProps,
  type IDockviewPanelProps,
  type IWatermarkPanelProps,
  type SerializedDockview,
} from 'dockview-react'
import { createContext, useCallback, useContext, useEffect, useRef } from 'react'
import { useMonitorStore } from '../../store/monitor-store'
import { MonitorPane } from './MonitorPane'
import { RichTerminalPane } from './RichTerminalPane'
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
  verboseMode: boolean
  onSelectSession?: (id: string) => void
}

// --- Panel component rendered inside each dockview panel ---

function SessionPanel({
  params,
}: IDockviewPanelProps<{
  sessionId: string
  verboseMode: boolean
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
      <RichTerminalPane sessionId={sessionId} isVisible={true} verboseMode={params.verboseMode} />
    </MonitorPane>
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

/**
 * Maps LiveSession.status to design-token status colors.
 * Values from packages/design-tokens/src/colors.ts -> status.
 */
function statusToColor(status: LiveSession['status']): string {
  switch (status) {
    case 'working':
      return '#22c55e' // status.active
    case 'paused':
      return '#f59e0b' // status.waiting
    case 'done':
      return '#6b7280' // status.done
    default:
      return '#6b7280'
  }
}

function SessionTabRenderer({
  api,
  containerApi: _containerApi,
  params,
}: IDockviewPanelHeaderProps) {
  const status = params.status as LiveSession['status'] | undefined
  const statusColor = status ? statusToColor(status) : '#6b7280'

  const handleMiddleClick = (e: React.MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault()
      e.stopPropagation()
      api.close()
    }
  }

  return (
    <div className="flex items-center gap-1.5 px-3 h-full text-xs" onMouseDown={handleMiddleClick}>
      <div
        className="w-1.5 h-1.5 rounded-full flex-shrink-0"
        style={{ backgroundColor: statusColor }}
      />
      <span className="truncate">{api.title}</span>
    </div>
  )
}

export function DockLayout({
  sessions,
  initialLayout,
  onLayoutChange,
  onApiReady,
  compactHeaders: _compactHeaders,
  verboseMode,
  onSelectSession,
}: DockLayoutProps) {
  const apiRef = useRef<DockviewApi | null>(null)
  const sessionsRef = useRef(sessions)
  sessionsRef.current = sessions
  const verboseModeRef = useRef(verboseMode)
  verboseModeRef.current = verboseMode
  const onLayoutChangeRef = useRef(onLayoutChange)
  onLayoutChangeRef.current = onLayoutChange

  // onReady fires ONCE when dockview mounts. All mutable values (sessions,
  // verboseMode) are read via refs so the callback identity is stable and
  // dockview never re-initializes on SSE ticks.
  const onReady = useCallback(
    (event: DockviewReadyEvent) => {
      apiRef.current = event.api
      onApiReady?.(event.api)

      const currentSessions = sessionsRef.current
      const currentVerbose = verboseModeRef.current

      let restored = false
      if (initialLayout) {
        try {
          // Restore saved layout
          event.api.fromJSON(initialLayout)
          restored = true
          // Update panel params with current verboseMode
          for (const panel of event.api.panels) {
            const session = currentSessions.find((s) => s.id === panel.id)
            if (session) {
              panel.api.updateParameters({
                sessionId: session.id,
                verboseMode: currentVerbose,
                status: session.status,
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
            title: session?.projectDisplayName ?? id.slice(0, 8),
            params: {
              sessionId: id,
              verboseMode: currentVerbose,
              status: session?.status ?? 'done',
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

  // Sync session data into existing panels when sessions update
  useEffect(() => {
    const api = apiRef.current
    if (!api) return

    // Update existing panels with fresh verboseMode
    for (const panel of api.panels) {
      const session = sessions.find((s) => s.id === panel.id)
      if (session) {
        panel.api.updateParameters({
          sessionId: session.id,
          verboseMode,
          status: session.status,
        })
      }
    }

    // Add panels for new sessions
    const existingIds = new Set(api.panels.map((p) => p.id))
    for (const session of sessions) {
      if (!existingIds.has(session.id)) {
        api.addPanel({
          id: session.id,
          component: 'session',
          title: session.projectDisplayName ?? session.id.slice(0, 8),
          params: {
            sessionId: session.id,
            verboseMode,
            status: session.status,
          },
        })
      }
    }

    // Remove panels for ended sessions.
    // IMPORTANT: Snapshot the array first — calling removePanel() mutates
    // api.panels in place, which causes iterator invalidation if we iterate
    // the live array directly (same pattern as Array.prototype.filter-then-forEach).
    const currentIds = new Set(sessions.map((s) => s.id))
    const panelsToRemove = api.panels.filter((p) => !currentIds.has(p.id))
    for (const panel of panelsToRemove) {
      api.removePanel(panel)
    }
  }, [sessions, verboseMode])

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
          className="dockview-theme-cv"
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
