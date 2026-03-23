import type { DockviewApi } from 'dockview-react'
import { useCallback, useMemo, useRef, useState } from 'react'
import { useLayoutMode } from '../../hooks/use-layout-mode'
import { useLayoutPresets } from '../../hooks/use-layout-presets'
import { useMonitorStore } from '../../store/monitor-store'
import { DockLayout } from './DockLayout'
import { GridControls } from './GridControls'
import { LayoutModeToggle } from './LayoutModeToggle'
import { LayoutPresets } from './LayoutPresets'
import { MonitorGrid } from './MonitorGrid'
import { MonitorPane } from './MonitorPane'
import { PaneContextMenu } from './PaneContextMenu'
import { BlockTerminalPane } from './BlockTerminalPane'
import type { LiveSession } from './use-live-sessions'
import { useAutoFill } from './useAutoFill'
import { useMonitorKeyboardShortcuts } from './useMonitorKeyboardShortcuts'

interface MonitorViewProps {
  sessions: LiveSession[]
  onSelectSession?: (id: string) => void
}

/**
 * MonitorView — orchestrates the full Monitor Mode experience.
 *
 * Wires together: MonitorGrid, GridControls, MonitorPane, BlockTerminalPane,
 * PaneContextMenu, keyboard shortcuts, and auto-fill.
 */
export function MonitorView({ sessions, onSelectSession }: MonitorViewProps) {
  // Store state
  const gridOverride = useMonitorStore((s) => s.gridOverride)
  const compactHeaders = useMonitorStore((s) => s.compactHeaders)
  const selectedPaneId = useMonitorStore((s) => s.selectedPaneId)
  const expandedPaneId = useMonitorStore((s) => s.expandedPaneId)
  const pinnedPaneIds = useMonitorStore((s) => s.pinnedPaneIds)
  const hiddenPaneIds = useMonitorStore((s) => s.hiddenPaneIds)
  const displayMode = useMonitorStore((s) => s.displayMode)

  // Store actions
  const setGridOverride = useMonitorStore((s) => s.setGridOverride)
  const setCompactHeaders = useMonitorStore((s) => s.setCompactHeaders)
  const selectPane = useMonitorStore((s) => s.selectPane)
  const expandPane = useMonitorStore((s) => s.expandPane)
  const pinPane = useMonitorStore((s) => s.pinPane)
  const unpinPane = useMonitorStore((s) => s.unpinPane)
  const hidePane = useMonitorStore((s) => s.hidePane)
  const setDisplayMode = useMonitorStore((s) => s.setDisplayMode)

  // Phase E: layout mode + presets
  const { mode, setMode, toggleMode, savedLayout, setSavedLayout, activePreset, setActivePreset } =
    useLayoutMode()
  const { customPresets, savePreset, deletePreset } = useLayoutPresets()
  const dockviewApiRef = useRef<DockviewApi | null>(null)

  // Visibility tracking from MonitorGrid's IntersectionObserver
  const [visiblePanes, setVisiblePanes] = useState<Set<string>>(new Set())

  // Context menu state
  const [contextMenu, setContextMenu] = useState<{
    x: number
    y: number
    sessionId: string
  } | null>(null)

  // Custom session ordering for "Move to front"
  const [sessionOrder, setSessionOrder] = useState<string[]>([])

  // Stable pane ordering — tracks first-seen order of session IDs.
  // The upstream `sessions` array is sorted by lastActivityAt (most recently active
  // first), which is correct for list/grid views but causes constant pane shuffling
  // in monitor view. Each shuffle repositions DOM nodes, causing xterm terminals to
  // flicker and potentially lose scroll state. This ref preserves insertion order:
  // existing sessions keep their position, new sessions are appended at the end.
  const stableOrderRef = useRef<string[]>([])

  // Calculate grid capacity — how many panes actually fit on screen
  const gridCapacity = useMemo(() => {
    if (gridOverride) return gridOverride.cols * gridOverride.rows
    // Auto mode: conservative default based on typical screen sizes
    // CSS grid uses minmax(480px, 1fr) cols and minmax(300px, 1fr) rows
    // Most screens fit 2-3 cols × 2-3 rows = 4-9 panes
    return 8
  }, [gridOverride])

  // Filter out hidden sessions, apply stable ordering, and limit to grid capacity
  const visibleSessions = useMemo(() => {
    const filtered = sessions.filter((s) => !hiddenPaneIds.has(s.id))
    const currentIds = new Set(filtered.map((s) => s.id))

    // Maintain stable order: prune departed sessions, append newly seen ones
    const kept = stableOrderRef.current.filter((id) => currentIds.has(id))
    const keptSet = new Set(kept)
    for (const s of filtered) {
      if (!keptSet.has(s.id)) kept.push(s.id)
    }
    stableOrderRef.current = kept

    // Layer manual "Move to front" ordering on top of stable base
    let order = kept
    if (sessionOrder.length > 0) {
      const manualSet = new Set(sessionOrder)
      const manual = sessionOrder.filter((id) => currentIds.has(id))
      const rest = kept.filter((id) => !manualSet.has(id))
      order = [...manual, ...rest]
    }

    // Sort filtered sessions by their position in the computed order
    const posMap = new Map(order.map((id, i) => [id, i]))
    return [...filtered]
      .sort((a, b) => (posMap.get(a.id) ?? 0) - (posMap.get(b.id) ?? 0))
      .slice(0, gridCapacity)
  }, [sessions, hiddenPaneIds, sessionOrder, gridCapacity])

  // Auto-fill: auto-show new sessions and swap idle ones out
  useAutoFill({ sessions, enabled: true })

  // Keyboard shortcuts — active when monitor view is showing
  useMonitorKeyboardShortcuts({
    enabled: true,
    sessions: visibleSessions,
    onLayoutModeChange: setMode,
    layoutMode: mode,
    dockviewApi: dockviewApiRef.current,
  })

  // Handlers
  const handleSelect = useCallback(
    (id: string) => {
      selectPane(selectedPaneId === id ? null : id)
    },
    [selectPane, selectedPaneId],
  )

  const handleExpand = useCallback(
    (id: string) => {
      if (onSelectSession) {
        onSelectSession(id)
      } else {
        expandPane(expandedPaneId === id ? null : id)
      }
    },
    [onSelectSession, expandPane, expandedPaneId],
  )

  const handlePin = useCallback(
    (id: string) => {
      if (pinnedPaneIds.has(id)) {
        unpinPane(id)
      } else {
        pinPane(id)
      }
    },
    [pinnedPaneIds, pinPane, unpinPane],
  )

  const handleContextMenu = useCallback((e: React.MouseEvent, sessionId: string) => {
    e.preventDefault()
    setContextMenu({ x: e.clientX, y: e.clientY, sessionId })
  }, [])

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null)
  }, [])

  // Phase E handlers
  const handleSelectPreset = useCallback(
    (presetName: string) => {
      setActivePreset(presetName)
    },
    [setActivePreset],
  )

  const handleSavePreset = useCallback(
    (name: string) => {
      if (dockviewApiRef.current) {
        savePreset(name, dockviewApiRef.current.toJSON())
      }
    },
    [savePreset],
  )

  const handleDeletePreset = useCallback(
    (name: string) => {
      deletePreset(name)
    },
    [deletePreset],
  )

  const handleResetLayout = useCallback(() => {
    setSavedLayout(null)
    setMode('auto-grid')
  }, [setSavedLayout, setMode])

  // Context menu session
  const contextMenuSession = contextMenu
    ? (sessions.find((s) => s.id === contextMenu.sessionId) ?? null)
    : null

  return (
    <div className="flex flex-col h-full gap-2">
      {/* Grid controls toolbar — existing Phase C (shown in both modes) */}
      <GridControls
        gridOverride={gridOverride}
        compactHeaders={compactHeaders}
        displayMode={displayMode}
        sessionCount={sessions.length}
        visibleCount={visibleSessions.length}
        onGridOverrideChange={setGridOverride}
        onCompactHeadersChange={setCompactHeaders}
        onDisplayModeChange={setDisplayMode}
      />

      {/* Layout mode toggle + preset controls (Phase E) */}
      <div className="flex items-center gap-3 px-3">
        <LayoutModeToggle mode={mode} onToggle={toggleMode} />

        {mode === 'custom' && (
          <>
            <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
            <LayoutPresets
              sessions={visibleSessions}
              dockviewApi={dockviewApiRef.current}
              activePreset={activePreset}
              onSelectPreset={handleSelectPreset}
              onSavePreset={handleSavePreset}
              onDeletePreset={handleDeletePreset}
              customPresets={customPresets}
              displayMode={displayMode}
            />
            <button
              type="button"
              onClick={handleResetLayout}
              className="text-xs text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 px-2 py-1 rounded border border-transparent hover:border-gray-300 dark:hover:border-gray-700 transition-colors"
            >
              Reset
            </button>
          </>
        )}
      </div>

      {/* Content area — conditional render */}
      <div className="flex-1 min-h-0 relative">
        {mode === 'auto-grid' ? (
          <MonitorGrid
            sessions={visibleSessions}
            gridOverride={gridOverride}
            compactHeaders={compactHeaders}
            onVisibilityChange={setVisiblePanes}
          >
            {visibleSessions.map((session) => {
              const isPinned = pinnedPaneIds.has(session.id)
              const isPaneVisible = visiblePanes.size === 0 || visiblePanes.has(session.id)

              return (
                <div key={session.id} data-pane-id={session.id}>
                  <MonitorPane
                    session={session}
                    isSelected={selectedPaneId === session.id}
                    isExpanded={false}
                    isPinned={isPinned}
                    compactHeader={compactHeaders}
                    isVisible={isPaneVisible}
                    onSelect={() => handleSelect(session.id)}
                    onExpand={() => handleExpand(session.id)}
                    onPin={() => handlePin(session.id)}
                    onHide={() => hidePane(session.id)}
                    onContextMenu={(e) => handleContextMenu(e, session.id)}
                  >
                    <BlockTerminalPane sessionId={session.id} isVisible={isPaneVisible} />
                  </MonitorPane>
                </div>
              )
            })}
          </MonitorGrid>
        ) : (
          <DockLayout
            sessions={visibleSessions}
            initialLayout={savedLayout}
            onLayoutChange={setSavedLayout}
            onApiReady={(api) => {
              dockviewApiRef.current = api
            }}
            compactHeaders={compactHeaders}
            displayMode={displayMode}
            onSelectSession={onSelectSession}
          />
        )}
      </div>

      {/* Context menu */}
      {contextMenu && contextMenuSession && (
        <PaneContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          sessionId={contextMenu.sessionId}
          isPinned={pinnedPaneIds.has(contextMenu.sessionId)}
          onClose={handleCloseContextMenu}
          onPin={() => {
            pinPane(contextMenu.sessionId)
            handleCloseContextMenu()
          }}
          onUnpin={() => {
            unpinPane(contextMenu.sessionId)
            handleCloseContextMenu()
          }}
          onHide={() => {
            hidePane(contextMenu.sessionId)
            handleCloseContextMenu()
          }}
          onMoveToFront={() => {
            const id = contextMenu.sessionId
            setSessionOrder((prev) => [id, ...prev.filter((x) => x !== id)])
            selectPane(id)
            handleCloseContextMenu()
          }}
          onExpand={() => {
            if (onSelectSession) {
              onSelectSession(contextMenu.sessionId)
            } else {
              expandPane(contextMenu.sessionId)
            }
            handleCloseContextMenu()
          }}
        />
      )}
    </div>
  )
}
