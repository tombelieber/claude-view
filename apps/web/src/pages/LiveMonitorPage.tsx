import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useOutletContext, useSearchParams } from 'react-router-dom'
import { LiveMonitorSkeleton } from '../components/LoadingStates'
import { CostTokenPopover } from '../components/live/CostTokenPopover'
import { KanbanView } from '../components/live/KanbanView'
import { KeyboardShortcutHelp } from '../components/live/KeyboardShortcutHelp'
import { ListView } from '../components/live/ListView'
import { LiveFilterBar } from '../components/live/LiveFilterBar'
import { MobileTabBar } from '../components/live/MobileTabBar'
import { MonitorView } from '../components/live/MonitorView'
import { OAuthUsagePill } from '../components/live/OAuthUsagePill'
import { SessionCard } from '../components/live/SessionCard'
import { SessionDetailPanel } from '../components/live/SessionDetailPanel'
import { TerminalOverlay } from '../components/live/TerminalOverlay'
import { ViewModeSwitcher } from '../components/live/ViewModeSwitcher'
import { filterLiveSessions } from '../components/live/live-filter'
import type { KanbanGroupBy, KanbanSort } from '../components/live/types'
import type { LiveViewMode } from '../components/live/types'
import { LIVE_VIEW_STORAGE_KEY } from '../components/live/types'
import { useKanbanGrouping } from '../components/live/use-kanban-grouping'
import { useKeyboardShortcuts } from '../components/live/use-keyboard-shortcuts'
import { useLiveSessionFilters } from '../components/live/use-live-session-filters'
import {
  type LiveSession,
  type LiveSummary,
  type UseLiveSessionsResult,
  sessionTotalCost,
} from '../components/live/use-live-sessions'
import type { IndexingProgress } from '../hooks/use-indexing-progress'
import { useLiveCommandStore } from '../store/live-command-context'
import { useMonitorStore } from '../store/monitor-store'

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
  const { liveSessions, indexingProgress } = useOutletContext<{
    liveSessions: UseLiveSessionsResult
    indexingProgress?: IndexingProgress
  }>()
  const {
    sessions,
    summary: serverSummary,
    isConnected,
    isInitialized,
    lastUpdate,
    stalledSessions,
    currentTime,
  } = liveSessions
  const [searchParams, setSearchParams] = useSearchParams()
  const navigate = useNavigate()
  const [viewMode, setViewMode] = useState<LiveViewMode>(() => resolveInitialView(searchParams))
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [monitorOverlayId, setMonitorOverlayId] = useState<string | null>(null)
  const [showHelp, setShowHelp] = useState(false)
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const setLiveContext = useLiveCommandStore((s) => s.setContext)

  // ?focus=<sessionId> handler — deep-link from HistoryView resume
  const selectPane = useMonitorStore((s) => s.selectPane)
  const focusSessionId = searchParams.get('focus')

  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally minimal deps — only fire when focusSessionId changes
  useEffect(() => {
    if (focusSessionId) {
      if (viewMode !== 'monitor') {
        handleViewModeChange('monitor')
      }
      selectPane(focusSessionId)
      const params = new URLSearchParams(searchParams)
      params.delete('focus')
      setSearchParams(params, { replace: true })
    }
  }, [focusSessionId])

  // Filters
  const [filters, filterActions] = useLiveSessionFilters(searchParams, setSearchParams)

  // Filtered sessions
  const filteredSessions = useMemo(() => filterLiveSessions(sessions, filters), [sessions, filters])

  // Kanban grouping
  const urlGroupBy = searchParams.get('groupBy') as KanbanGroupBy | null
  const { groupBy, setGroupBy, sort, setSort, projectGroups, isCollapsed, toggleCollapse } =
    useKanbanGrouping(filteredSessions, urlGroupBy)

  const handleGroupByChange = useCallback(
    (value: KanbanGroupBy) => {
      setGroupBy(value)
      const params = new URLSearchParams(searchParams)
      if (value === 'none') {
        params.delete('groupBy')
      } else {
        params.set('groupBy', value)
      }
      setSearchParams(params, { replace: true })
    },
    [searchParams, setSearchParams, setGroupBy],
  )

  const handleSortChange = useCallback(
    (value: KanbanSort) => {
      setSort(value)
    },
    [setSort],
  )

  // Derive summary from current sessions so it always matches the kanban/grid.
  // Server-side summary is only used for cost/tokens (which may include cleaned-up sessions).
  const summary = useMemo<LiveSummary | null>(() => {
    if (sessions.length === 0 && !serverSummary) return null
    let needsYouCount = 0
    let autonomousCount = 0
    let totalCostTodayUsd = 0
    let totalTokensToday = 0
    let inputTokens = 0
    let outputTokens = 0
    let cacheReadTokens = 0
    let cacheCreationTokens = 0
    let inputCostUsd = 0
    let outputCostUsd = 0
    let cacheReadCostUsd = 0
    let cacheCreationCostUsd = 0
    let cacheSavingsUsd = 0
    for (const s of sessions) {
      switch (s.agentState.group) {
        case 'needs_you':
          needsYouCount++
          break
        case 'autonomous':
          autonomousCount++
          break
      }
      totalCostTodayUsd += sessionTotalCost(s)
      totalTokensToday += s.tokens?.totalTokens ?? 0
      inputTokens += s.tokens?.inputTokens ?? 0
      outputTokens += s.tokens?.outputTokens ?? 0
      cacheReadTokens += s.tokens?.cacheReadTokens ?? 0
      cacheCreationTokens += s.tokens?.cacheCreationTokens ?? 0
      inputCostUsd += s.cost?.inputCostUsd ?? 0
      outputCostUsd += s.cost?.outputCostUsd ?? 0
      cacheReadCostUsd += s.cost?.cacheReadCostUsd ?? 0
      cacheCreationCostUsd += s.cost?.cacheCreationCostUsd ?? 0
      cacheSavingsUsd += s.cost?.cacheSavingsUsd ?? 0
    }
    return {
      needsYouCount,
      autonomousCount,
      totalCostTodayUsd,
      totalTokensToday,
      processCount: serverSummary?.processCount ?? sessions.length,
      inputTokens,
      outputTokens,
      cacheReadTokens,
      cacheCreationTokens,
      inputCostUsd,
      outputCostUsd,
      cacheReadCostUsd,
      cacheCreationCostUsd,
      cacheSavingsUsd,
    }
  }, [sessions, serverSummary])

  // Available filter options from current (unfiltered) sessions
  const availableStatuses = useMemo(() => {
    const set = new Set(sessions.map((s) => s.agentState.group))
    return Array.from(set)
  }, [sessions])

  const availableProjects = useMemo(() => {
    const set = new Set(sessions.map((s) => s.projectDisplayName || s.project))
    return Array.from(set).sort()
  }, [sessions])

  const availableBranches = useMemo(() => {
    const set = new Set(sessions.filter((s) => s.effectiveBranch).map((s) => s.effectiveBranch!))
    return Array.from(set).sort()
  }, [sessions])

  // View mode change — clear overlays when switching views
  const handleViewModeChange = useCallback(
    (mode: LiveViewMode) => {
      setViewMode(mode)
      setSelectedId(null)
      setMonitorOverlayId(null)
      localStorage.setItem(LIVE_VIEW_STORAGE_KEY, mode)
      const params = new URLSearchParams(searchParams)
      params.set('view', mode)
      setSearchParams(params, { replace: true })
    },
    [searchParams, setSearchParams],
  )

  // Session selection
  const handleSelectSession = useCallback((id: string) => {
    setSelectedId((prev) => (prev === id ? null : id))
  }, [])

  // Expand selected session (navigate to detail)
  const handleExpandSession = useCallback(
    (id: string) => {
      navigate(`/sessions/${id}`)
    },
    [navigate],
  )

  // Monitor view: open large terminal overlay instead of side panel
  const handleMonitorExpand = useCallback((id: string) => {
    setMonitorOverlayId((prev) => (prev === id ? null : id))
  }, [])

  // Toggle help callback for command palette
  const handleToggleHelp = useCallback(() => {
    setShowHelp((prev) => !prev)
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
  const commandContext = useMemo(
    () => ({
      sessions,
      viewMode,
      onViewModeChange: handleViewModeChange,
      onFilterStatus: filterActions.setStatus,
      onClearFilters: filterActions.clearAll,
      onSort: filterActions.setSort,
      onSelectSession: handleSelectSession,
      onToggleHelp: handleToggleHelp,
    }),
    [
      sessions,
      viewMode,
      handleViewModeChange,
      handleSelectSession,
      handleToggleHelp,
      filterActions.setStatus,
      filterActions.clearAll,
      filterActions.setSort,
    ],
  )

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

  // Show skeleton until the server's first summary event arrives.
  if (!isInitialized) {
    return <LiveMonitorSkeleton />
  }

  // "Updated Xs ago" — only show when stale (>30s) to reduce noise
  const isStale = lastUpdate ? Date.now() - lastUpdate.getTime() > 30_000 : false

  // Compute indexing percent once for filter bar
  const indexingPercent = indexingProgress
    ? indexingProgress.bytesTotal > 0
      ? Math.min(
          100,
          Math.round((indexingProgress.bytesProcessed / indexingProgress.bytesTotal) * 100),
        )
      : indexingProgress.total > 0
        ? Math.min(100, Math.round((indexingProgress.indexed / indexingProgress.total) * 100))
        : 0
    : 0

  return (
    <div className="h-full flex flex-col">
      {/* Pinned header — never scrolls */}
      <div className="shrink-0 px-6 pt-4 space-y-3">
        <div className="max-w-7xl mx-auto space-y-3">
          {/* Row 1: Command bar — title, view tabs, stats, status */}
          <div className="flex items-center gap-3">
            {/* Left: status dot + title + view tabs + session counts */}
            <div className="flex items-center gap-3 min-w-0">
              <div className="flex items-center gap-2">
                <span
                  className={`inline-block h-2 w-2 rounded-full shrink-0 ${isConnected ? 'bg-green-500' : 'bg-red-500 animate-pulse'}`}
                  title={isConnected ? 'Connected — live updates' : 'Reconnecting...'}
                />
                <h1 className="text-base font-semibold text-gray-900 dark:text-gray-100 whitespace-nowrap">
                  Live Monitor
                </h1>
              </div>
              <ViewModeSwitcher mode={viewMode} onChange={handleViewModeChange} />

              {/* Session count badges — stick with title group */}
              {summary && (
                <div className="flex items-center gap-2 text-xs">
                  <span
                    className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full font-medium tabular-nums ${
                      summary.needsYouCount > 0
                        ? 'bg-amber-500/15 text-amber-500'
                        : 'text-gray-400 dark:text-gray-500'
                    }`}
                  >
                    {summary.needsYouCount > 0 && (
                      <span className="inline-block h-1.5 w-1.5 rounded-full bg-amber-500 animate-pulse" />
                    )}
                    <span>{summary.needsYouCount}</span>
                    <span className="font-normal opacity-70">needs you</span>
                  </span>

                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-green-500 font-medium tabular-nums">
                    <span>{summary.autonomousCount}</span>
                    <span className="font-normal opacity-70">autonomous</span>
                  </span>
                </div>
              )}
            </div>

            {/* Right: cost, tokens, stale indicator, usage — stays right-aligned */}
            <div className="flex items-center gap-3 ml-auto text-xs shrink-0">
              {isStale && lastUpdate && (
                <span className="text-amber-500/80 tabular-nums">
                  {formatRelativeTime(lastUpdate)}
                </span>
              )}

              {summary && <CostTokenPopover summary={summary} />}

              <OAuthUsagePill />
            </div>
          </div>

          {/* Row 2: Filter bar */}
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
            indexingPhase={indexingProgress?.phase}
            indexingPercent={indexingPercent}
            filteredCount={filteredSessions.length}
            totalCount={sessions.length}
            groupByValue={viewMode === 'kanban' ? groupBy : undefined}
            onGroupByChange={viewMode === 'kanban' ? handleGroupByChange : undefined}
            sortValue={viewMode === 'kanban' ? sort : undefined}
            onSortChange={viewMode === 'kanban' ? handleSortChange : undefined}
          />
        </div>
      </div>

      {/* Scrollable content — kanban columns scroll independently, other views page-scroll */}
      <div
        className={`flex-1 min-h-0 px-6 pt-4 pb-20 sm:pb-6 ${viewMode === 'kanban' || viewMode === 'monitor' ? 'flex flex-col overflow-hidden' : 'overflow-y-auto'}`}
      >
        <div
          className={`max-w-7xl mx-auto w-full ${viewMode === 'kanban' || viewMode === 'monitor' ? 'flex-1 min-h-0 flex flex-col' : ''}`}
        >
          {/* View content */}
          {viewMode === 'grid' && (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
              {filteredSessions.map((session) => (
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
            <ListView
              sessions={filteredSessions}
              selectedId={selectedId}
              onSelect={handleSelectSession}
            />
          )}

          {viewMode === 'kanban' && (
            <KanbanView
              sessions={filteredSessions}
              selectedId={selectedId}
              onSelect={handleSelectSession}
              onCardClick={handleSelectSession}
              stalledSessions={stalledSessions}
              currentTime={currentTime}
              groupBy={groupBy}
              projectGroups={projectGroups}
              isCollapsed={isCollapsed}
              toggleCollapse={toggleCollapse}
            />
          )}

          {viewMode === 'monitor' && (
            <MonitorView sessions={filteredSessions} onSelectSession={handleMonitorExpand} />
          )}

          {/* Empty state — skip for kanban (columns have their own emptyMessage) */}
          {filteredSessions.length === 0 && isConnected && viewMode !== 'kanban' && (
            <div className="text-center text-gray-400 dark:text-gray-500 py-16">
              <div className="text-4xl mb-4">~</div>
              {sessions.length === 0 ? (
                serverSummary && serverSummary.processCount > 0 ? (
                  <>
                    <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-500/10 text-green-500 text-sm font-medium mb-3">
                      <span className="inline-block h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                      {serverSummary.processCount} Claude{' '}
                      {serverSummary.processCount === 1 ? 'process' : 'processes'} detected
                    </div>
                    <div className="text-sm">Sessions appear as they report in via hooks.</div>
                    <div className="text-xs mt-1 text-gray-500 dark:text-gray-600">
                      Try sending a message in one of your Claude Code terminals.
                    </div>
                  </>
                ) : (
                  <>
                    <div className="text-sm">No active Claude Code sessions detected.</div>
                    <div className="text-xs mt-1">
                      Start a session in your terminal and it will appear here.
                    </div>
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
      {selectedId &&
        (() => {
          const session = sessions.find((s) => s.id === selectedId)
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
      {monitorOverlayId &&
        (() => {
          const session = sessions.find((s) => s.id === monitorOverlayId)
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

function formatRelativeTime(date: Date): string {
  const diff = (Date.now() - date.getTime()) / 1000
  if (diff < 5) return 'just now'
  if (diff < 60) return `${Math.floor(diff)}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  return `${Math.floor(diff / 3600)}h ago`
}
