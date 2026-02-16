import { useState, useCallback, useMemo, useRef } from 'react'
import type { LiveSession } from './use-live-sessions'
import { useMonitorStore } from '../../store/monitor-store'
import { MonitorGrid } from './MonitorGrid'
import { GridControls } from './GridControls'
import { MonitorPane } from './MonitorPane'
import { RichPane, parseRichMessage, type RichMessage } from './RichPane'
import { ExpandedPaneOverlay } from './ExpandedPaneOverlay'
import { PaneContextMenu } from './PaneContextMenu'
import { useMonitorKeyboardShortcuts } from './useMonitorKeyboardShortcuts'
import { useAutoFill } from './useAutoFill'
import { useTerminalSocket, type ConnectionState } from '../../hooks/use-terminal-socket'
import { SwimLanes } from './SwimLanes'

interface MonitorViewProps {
  sessions: LiveSession[]
}

/**
 * RichTerminalPane — wraps useTerminalSocket + RichPane for rich mode.
 * Manages its own WebSocket connection and parses messages into RichMessage[].
 */
function RichTerminalPane({ sessionId, isVisible, verboseMode }: { sessionId: string; isVisible: boolean; verboseMode: boolean }) {
  const [messages, setMessages] = useState<RichMessage[]>([])
  const [bufferDone, setBufferDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
    const parsed = parseRichMessage(data)
    if (parsed) {
      setMessages((prev) => [...prev, parsed])
    }
  }, [])

  const handleConnectionChange = useCallback((state: ConnectionState) => {
    if (state === 'connected') {
      setBufferDone(true)
    }
  }, [])

  useTerminalSocket({
    sessionId,
    mode: 'rich',
    enabled: isVisible,
    onMessage: handleMessage,
    onConnectionChange: handleConnectionChange,
  })

  return <RichPane messages={messages} isVisible={isVisible} verboseMode={verboseMode} bufferDone={bufferDone} />
}

/**
 * MonitorView — orchestrates the full Monitor Mode experience.
 *
 * Wires together: MonitorGrid, GridControls, MonitorPane, RichPane,
 * ExpandedPaneOverlay, PaneContextMenu, keyboard shortcuts, and auto-fill.
 */
export function MonitorView({ sessions }: MonitorViewProps) {
  // Store state
  const gridOverride = useMonitorStore((s) => s.gridOverride)
  const compactHeaders = useMonitorStore((s) => s.compactHeaders)
  const selectedPaneId = useMonitorStore((s) => s.selectedPaneId)
  const expandedPaneId = useMonitorStore((s) => s.expandedPaneId)
  const pinnedPaneIds = useMonitorStore((s) => s.pinnedPaneIds)
  const hiddenPaneIds = useMonitorStore((s) => s.hiddenPaneIds)
  const verboseMode = useMonitorStore((s) => s.verboseMode)

  // Store actions
  const setGridOverride = useMonitorStore((s) => s.setGridOverride)
  const setCompactHeaders = useMonitorStore((s) => s.setCompactHeaders)
  const selectPane = useMonitorStore((s) => s.selectPane)
  const expandPane = useMonitorStore((s) => s.expandPane)
  const pinPane = useMonitorStore((s) => s.pinPane)
  const unpinPane = useMonitorStore((s) => s.unpinPane)
  const hidePane = useMonitorStore((s) => s.hidePane)
  const toggleVerbose = useMonitorStore((s) => s.toggleVerbose)

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
  useMonitorKeyboardShortcuts({ enabled: true, sessions: visibleSessions })

  // Handlers
  const handleSelect = useCallback(
    (id: string) => {
      selectPane(selectedPaneId === id ? null : id)
    },
    [selectPane, selectedPaneId]
  )

  const handleExpand = useCallback(
    (id: string) => {
      expandPane(expandedPaneId === id ? null : id)
    },
    [expandPane, expandedPaneId]
  )

  const handlePin = useCallback(
    (id: string) => {
      if (pinnedPaneIds.has(id)) {
        unpinPane(id)
      } else {
        pinPane(id)
      }
    },
    [pinnedPaneIds, pinPane, unpinPane]
  )

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, sessionId: string) => {
      e.preventDefault()
      setContextMenu({ x: e.clientX, y: e.clientY, sessionId })
    },
    []
  )

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null)
  }, [])

  // Expanded session
  const expandedSession = expandedPaneId
    ? sessions.find((s) => s.id === expandedPaneId) ?? null
    : null

  // Context menu session
  const contextMenuSession = contextMenu
    ? sessions.find((s) => s.id === contextMenu.sessionId) ?? null
    : null

  return (
    <div className="flex flex-col h-full gap-2">
      {/* Grid controls toolbar */}
      <GridControls
        gridOverride={gridOverride}
        compactHeaders={compactHeaders}
        verboseMode={verboseMode}
        sessionCount={sessions.length}
        visibleCount={visibleSessions.length}
        onGridOverrideChange={setGridOverride}
        onCompactHeadersChange={setCompactHeaders}
        onVerboseModeChange={toggleVerbose}
      />

      {/* Monitor grid */}
      <div className="flex-1 min-h-0">
        <MonitorGrid
          sessions={visibleSessions}
          gridOverride={gridOverride}
          compactHeaders={compactHeaders}
          onVisibilityChange={setVisiblePanes}
        >
          {visibleSessions.map((session) => {
            const isPinned = pinnedPaneIds.has(session.id)
            // Default to visible when observer hasn't fired yet (size === 0) since we've
            // already limited the number of panes to gridCapacity above.
            // When a pane is expanded in the overlay, disconnect the grid pane's WebSocket
            // to avoid doubling connections (overlay creates its own RichTerminalPane).
            const isPaneVisible =
              expandedPaneId !== session.id &&
              (visiblePanes.size === 0 || visiblePanes.has(session.id))

            return (
              <div
                key={session.id}
                data-pane-id={session.id}
              >
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
                  <RichTerminalPane
                    sessionId={session.id}
                    isVisible={isPaneVisible}
                    verboseMode={verboseMode}
                  />
                </MonitorPane>
              </div>
            )
          })}
        </MonitorGrid>
      </div>

      {/* Expanded pane overlay */}
      {expandedSession && (
        <ExpandedPaneOverlay
          session={expandedSession}
          onClose={() => expandPane(null)}
        >
          <div className="flex flex-col h-full gap-3">
            {/* Sub-agent swim lanes */}
            {expandedSession.subAgents && expandedSession.subAgents.length > 0 && (
              <SwimLanes
                subAgents={expandedSession.subAgents}
                sessionActive={expandedSession.status === 'working'}
              />
            )}
            {/* Terminal stream */}
            <div className="flex-1 min-h-0">
              <RichTerminalPane
                sessionId={expandedSession.id}
                isVisible={true}
                verboseMode={verboseMode}
              />
            </div>
          </div>
        </ExpandedPaneOverlay>
      )}

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
            expandPane(contextMenu.sessionId)
            handleCloseContextMenu()
          }}
        />
      )}
    </div>
  )
}
