import { useState, useCallback, useEffect, useMemo, useRef } from 'react'
import { useSearchParams, useNavigate, useOutletContext } from 'react-router-dom'
import { useLiveSessionFilters } from '../components/live/use-live-session-filters'
import { useKeyboardShortcuts } from '../components/live/use-keyboard-shortcuts'
import { filterLiveSessions } from '../components/live/live-filter'
import { SessionCard } from '../components/live/SessionCard'
import { ViewModeSwitcher } from '../components/live/ViewModeSwitcher'
import { ListView } from '../components/live/ListView'
import { KanbanView } from '../components/live/KanbanView'
import { MonitorView } from '../components/live/MonitorView'
import { LiveFilterBar } from '../components/live/LiveFilterBar'
import { KeyboardShortcutHelp } from '../components/live/KeyboardShortcutHelp'
import { MobileTabBar } from '../components/live/MobileTabBar'
import { SessionDetailPanel } from '../components/live/SessionDetailPanel'
import { TerminalOverlay } from '../components/live/TerminalOverlay'
import { sessionTotalCost, type LiveSummary, type LiveSession, type UseLiveSessionsResult } from '../components/live/use-live-sessions'
import type { LiveViewMode } from '../components/live/types'
import { LIVE_VIEW_STORAGE_KEY } from '../components/live/types'
import { formatTokenCount } from '../lib/format-utils'
import { OAuthUsagePill } from '../components/live/OAuthUsagePill'
import { LiveMonitorSkeleton } from '../components/LoadingStates'
import { useLiveCommandStore } from '../store/live-command-context'

function resolveInitialView(searchParams: URLSearchParams): LiveViewMode {
  const urlView = searchParams.get('view') as LiveViewMode | null
  if (urlView && ['grid', 'list', 'kanban', 'monitor'].includes(urlView)) {
    return urlView
  }
  const stored = localStorage.getItem(LIVE_VIEW_STORAGE_KEY) as LiveViewMode | null
  if (stored && ['grid', 'list', 'kanban', 'monitor'].includes(stored)) {
    return stored
  }
  return 'kanban'
}

export function LiveMonitorPage() {
  const { liveSessions } = useOutletContext<{ liveSessions: UseLiveSessionsResult }>()
  const { sessions, summary: serverSummary, isConnected, lastUpdate, stalledSessions, currentTime } = liveSessions
  const [searchParams, setSearchParams] = useSearchParams()
  const navigate = useNavigate()
  const [viewMode, setViewMode] = useState<LiveViewMode>(() => resolveInitialView(searchParams))
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [monitorOverlayId, setMonitorOverlayId] = useState<string | null>(null)
  const [showHelp, setShowHelp] = useState(false)
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const setLiveContext = useLiveCommandStore((s) => s.setContext)

  // Filters
  const [filters, filterActions] = useLiveSessionFilters(searchParams, setSearchParams)

  // Filtered sessions
  const filteredSessions = useMemo(
    () => filterLiveSessions(sessions, filters),
    [sessions, filters]
  )

  // Derive summary from current sessions so it always matches the kanban/grid.
  // Server-side summary is only used for cost/tokens (which may include cleaned-up sessions).
  const summary = useMemo<LiveSummary | null>(() => {
    if (sessions.length === 0 && !serverSummary) return null
    let needsYouCount = 0
    let autonomousCount = 0
    let totalCostTodayUsd = 0
    let totalTokensToday = 0
    for (const s of sessions) {
      switch (s.agentState.group) {
        case 'needs_you': needsYouCount++; break
        case 'autonomous': autonomousCount++; break
      }
      totalCostTodayUsd += sessionTotalCost(s)
      totalTokensToday += s.tokens?.totalTokens ?? 0
    }
    return { needsYouCount, autonomousCount, totalCostTodayUsd, totalTokensToday, processCount: serverSummary?.processCount ?? sessions.length }
  }, [sessions, serverSummary])

  // Available filter options from current (unfiltered) sessions
  const availableStatuses = useMemo(() => {
    const set = new Set(sessions.map(s => s.agentState.group))
    return Array.from(set)
  }, [sessions])

  const availableProjects = useMemo(() => {
    const set = new Set(sessions.map(s => s.projectDisplayName || s.project))
    return Array.from(set).sort()
  }, [sessions])

  const availableBranches = useMemo(() => {
    const set = new Set(sessions.filter(s => s.gitBranch).map(s => s.gitBranch!))
    return Array.from(set).sort()
  }, [sessions])

  // View mode change — clear overlays when switching views
  const handleViewModeChange = useCallback((mode: LiveViewMode) => {
    setViewMode(mode)
    setSelectedId(null)
    setMonitorOverlayId(null)
    localStorage.setItem(LIVE_VIEW_STORAGE_KEY, mode)
    const params = new URLSearchParams(searchParams)
    params.set('view', mode)
    setSearchParams(params, { replace: true })
  }, [searchParams, setSearchParams])

  // Session selection
  const handleSelectSession = useCallback((id: string) => {
    setSelectedId(prev => prev === id ? null : id)
  }, [])

  // Expand selected session (navigate to detail)
  const handleExpandSession = useCallback((id: string) => {
    navigate(`/sessions/${id}`)
  }, [navigate])

  // Monitor view: open large terminal overlay instead of side panel
  const handleMonitorExpand = useCallback((id: string) => {
    setMonitorOverlayId(prev => prev === id ? null : id)
  }, [])

  // Toggle help callback for command palette
  const handleToggleHelp = useCallback(() => {
    setShowHelp(prev => !prev)
  }, [])

  // Keyboard shortcuts
  useKeyboardShortcuts({
    viewMode,
    onViewModeChange: handleViewModeChange,
    sessions: filteredSessions,
    selectedId,
    onSelect: setSelectedId,
    onExpand: handleExpandSession,
    onFocusSearch: () => searchInputRef.current?.focus(),
    onToggleHelp: handleToggleHelp,
    enabled: !showHelp,
  })

  // Register live command context for unified Cmd+K
  // Use ref to prevent infinite loops from context updates
  const contextRef = useRef<{ viewMode: LiveViewMode; sessions: LiveSession[] } | null>(null)
  const commandContext = useMemo(() => ({
    sessions,
    viewMode,
    onViewModeChange: handleViewModeChange,
    onFilterStatus: filterActions.setStatus,
    onClearFilters: filterActions.clearAll,
    onSort: filterActions.setSort,
    onSelectSession: handleSelectSession,
    onToggleHelp: handleToggleHelp,
  }), [sessions, viewMode, handleViewModeChange, handleSelectSession, handleToggleHelp, filterActions.setStatus, filterActions.clearAll, filterActions.setSort])

  useEffect(() => {
    // Only update if context actually changed (shallow compare key properties)
    const prev = contextRef.current
    if (
      prev?.viewMode !== commandContext.viewMode ||
      prev?.sessions.length !== commandContext.sessions.length
    ) {
      contextRef.current = commandContext
      setLiveContext(commandContext)
    }
  }, [commandContext, setLiveContext])

  useEffect(() => {
    return () => setLiveContext(null)
  }, [setLiveContext])

  // SSE not connected yet and no sessions — show skeleton instead of blank content
  if (!isConnected && sessions.length === 0) {
    return <LiveMonitorSkeleton />
  }

  return (
    <div className="h-full flex flex-col">
      {/* Pinned header — never scrolls */}
      <div className="flex-shrink-0 px-6 pt-6 space-y-4">
        <div className="max-w-7xl mx-auto space-y-4">
          {/* Header */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
                Live Monitor
              </h1>
              <ViewModeSwitcher mode={viewMode} onChange={handleViewModeChange} />
            </div>
            <div className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500">
              <span
                className={`inline-block h-2 w-2 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`}
              />
              {isConnected ? 'Live' : 'Reconnecting...'}
              {lastUpdate && (
                <span className="ml-2">
                  Updated {formatRelativeTime(lastUpdate)}
                </span>
              )}
              <OAuthUsagePill />
            </div>
          </div>

          {/* Summary bar */}
          <SummaryBar summary={summary} filteredCount={filteredSessions.length} totalCount={sessions.length} />

          {/* Filter bar */}
          <LiveFilterBar
            filters={filters}
            onStatusChange={filterActions.setStatus}
            onProjectChange={filterActions.setProjects}
            onBranchChange={filterActions.setBranches}
            onSearchChange={filterActions.setSearch}
            onClear={filterActions.clearAll}
            activeCount={filterActions.activeCount}
            availableStatuses={availableStatuses}
            availableProjects={availableProjects}
            availableBranches={availableBranches}
            searchInputRef={searchInputRef}
          />
        </div>
      </div>

      {/* Scrollable content — kanban columns scroll independently, other views page-scroll */}
      <div className={`flex-1 min-h-0 px-6 pt-4 pb-20 sm:pb-6 ${viewMode === 'kanban' ? 'flex flex-col overflow-hidden' : 'overflow-y-auto'}`}>
        <div className={`max-w-7xl mx-auto w-full ${viewMode === 'kanban' ? 'flex-1 min-h-0 flex flex-col' : ''}`}>
          {/* View content */}
          {viewMode === 'grid' && (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
              {filteredSessions.map(session => (
                <div
                  key={session.id}
                  data-session-id={session.id}
                  className={selectedId === session.id ? 'ring-2 ring-indigo-500 rounded-lg' : ''}
                >
                  <SessionCard
                    session={session}
                    stalledSessions={stalledSessions}
                    currentTime={currentTime}
                    onClickOverride={() => handleSelectSession(session.id)}
                  />
                </div>
              ))}
            </div>
          )}

          {viewMode === 'list' && (
            <ListView sessions={filteredSessions} selectedId={selectedId} onSelect={handleSelectSession} />
          )}

          {viewMode === 'kanban' && (
            <KanbanView
              sessions={filteredSessions}
              selectedId={selectedId}
              onSelect={handleSelectSession}
              onCardClick={handleSelectSession}
              stalledSessions={stalledSessions}
              currentTime={currentTime}
            />
          )}

          {viewMode === 'monitor' && (
            <MonitorView sessions={filteredSessions} onSelectSession={handleMonitorExpand} />
          )}

          {/* Empty state */}
          {filteredSessions.length === 0 && isConnected && (
            <div className="text-center text-gray-400 dark:text-gray-500 py-16">
              <div className="text-4xl mb-4">~</div>
              {sessions.length === 0 ? (
                serverSummary && serverSummary.processCount > 0 ? (
                  <>
                    <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-500/10 text-green-500 text-sm font-medium mb-3">
                      <span className="inline-block h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                      {serverSummary.processCount} Claude {serverSummary.processCount === 1 ? 'process' : 'processes'} detected
                    </div>
                    <div className="text-sm">Sessions appear as they report in via hooks.</div>
                    <div className="text-xs mt-1 text-gray-500 dark:text-gray-600">
                      Try sending a message in one of your Claude Code terminals.
                    </div>
                  </>
                ) : (
                  <>
                    <div className="text-sm">No active Claude Code sessions detected.</div>
                    <div className="text-xs mt-1">Start a session in your terminal and it will appear here.</div>
                  </>
                )
              ) : (
                <>
                  <div className="text-sm">No sessions match your filters.</div>
                  <div className="text-xs mt-1">
                    <button
                      type="button"
                      onClick={filterActions.clearAll}
                      className="text-indigo-400 hover:text-indigo-300"
                    >
                      Clear filters
                    </button>
                  </div>
                </>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Session detail panel (Grid / List / Kanban) */}
      {selectedId && (() => {
        const session = sessions.find(s => s.id === selectedId)
        if (!session) return null
        return (
          <SessionDetailPanel
            key={selectedId}
            session={session}
            onClose={() => setSelectedId(null)}
          />
        )
      })()}

      {/* Terminal overlay (Monitor view) */}
      {monitorOverlayId && (() => {
        const session = sessions.find(s => s.id === monitorOverlayId)
        if (!session) return null
        return (
          <TerminalOverlay
            key={monitorOverlayId}
            session={session}
            onClose={() => setMonitorOverlayId(null)}
          />
        )
      })()}

      {/* Mobile tab bar */}
      <MobileTabBar activeTab={viewMode} onTabChange={handleViewModeChange} />

      {/* Keyboard shortcut help */}
      <KeyboardShortcutHelp isOpen={showHelp} onClose={() => setShowHelp(false)} />
    </div>
  )
}

interface SummaryBarProps {
  summary: LiveSummary | null
  filteredCount: number
  totalCount: number
}

function SummaryBar({ summary, filteredCount, totalCount }: SummaryBarProps) {
  if (!summary) return null

  const showFiltered = filteredCount !== totalCount

  return (
    <div className="flex flex-wrap gap-x-6 gap-y-2 p-3 rounded-lg bg-gray-100/50 dark:bg-gray-800/50 text-sm">
      <div>
        <span className="text-amber-500 font-medium">{summary.needsYouCount}</span>
        <span className="text-gray-500 dark:text-gray-400 ml-1">needs you</span>
      </div>
      <div>
        <span className="text-green-500 font-medium">{summary.autonomousCount}</span>
        <span className="text-gray-500 dark:text-gray-400 ml-1">autonomous</span>
      </div>
      {showFiltered && (
        <div>
          <span className="text-indigo-400 font-medium">{filteredCount}</span>
          <span className="text-gray-500 dark:text-gray-400 ml-1">of {totalCount} shown</span>
        </div>
      )}
      <div className="ml-auto flex gap-4">
        <span className="text-gray-600 dark:text-gray-300 font-mono tabular-nums">
          ${summary.totalCostTodayUsd.toFixed(2)}
          <span className="text-gray-400 dark:text-gray-500 font-sans ml-1">today</span>
        </span>
        <span className="text-gray-600 dark:text-gray-300 font-mono tabular-nums">
          {formatTokenCount(summary.totalTokensToday)}
          <span className="text-gray-400 dark:text-gray-500 font-sans ml-1">tokens</span>
        </span>
      </div>
    </div>
  )
}

function formatRelativeTime(date: Date): string {
  const diff = (Date.now() - date.getTime()) / 1000
  if (diff < 5) return 'just now'
  if (diff < 60) return `${Math.floor(diff)}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  return `${Math.floor(diff / 3600)}h ago`
}
