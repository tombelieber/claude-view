// src/components/HistoryView.tsx

import { useState, useMemo, useRef, useEffect, useCallback } from 'react'
import { Link, useSearchParams, useNavigate } from 'react-router-dom'
import { Search, X, FolderOpen, ArrowLeft, Clock, TrendingUp, FileEdit, MessageSquare, Coins, ChevronDown } from 'lucide-react'
import { useProjectSummaries, useAllSessions } from '../hooks/use-projects'
import { SessionCard } from './SessionCard'
import { CompactSessionTable } from './CompactSessionTable'
import type { SortColumn } from './CompactSessionTable'
import { ActivitySparkline } from './ActivitySparkline'
import { SessionToolbar } from './SessionToolbar'
import { useSessionFilters, DEFAULT_FILTERS } from '../hooks/use-session-filters'
import type { SessionSort } from '../hooks/use-session-filters'
import { groupSessionsByDate } from '../lib/date-groups'
import { groupSessions, shouldDisableGrouping, MAX_GROUPABLE_SESSIONS } from '../utils/group-sessions'
import { sessionSlug } from '../lib/url-slugs'
import { Skeleton, SessionsEmptyState } from './LoadingStates'
import { cn } from '../lib/utils'

type TimeFilter = 'all' | 'today' | '7d' | '30d'

/** Human-readable labels for sort options */
const SORT_LABELS: Record<SessionSort, string> = {
  recent: 'Most recent',
  tokens: 'Most tokens',
  prompts: 'Most prompts',
  files_edited: 'Most files edited',
  duration: 'Longest duration',
}

const SORT_ICONS: Record<SessionSort, React.ReactNode> = {
  recent: <Clock className="w-3.5 h-3.5" />,
  tokens: <Coins className="w-3.5 h-3.5" />,
  prompts: <MessageSquare className="w-3.5 h-3.5" />,
  files_edited: <FileEdit className="w-3.5 h-3.5" />,
  duration: <Clock className="w-3.5 h-3.5" />,
}

/** Format duration in human-readable form */
function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = seconds / 3600
  return `${hours.toFixed(1)}h`
}

/** Format value for the sort metric displayed on each card */
function formatSortMetric(session: { durationSeconds?: number; userPromptCount?: number; filesEditedCount?: number; totalInputTokens?: bigint | null; totalOutputTokens?: bigint | null }, sort: SessionSort): string | null {
  switch (sort) {
    case 'duration': {
      const dur = session.durationSeconds ?? 0
      return dur > 0 ? formatDuration(dur) : null
    }
    case 'prompts': {
      const count = session.userPromptCount ?? 0
      return count > 0 ? `${count} prompts` : null
    }
    case 'files_edited': {
      const count = session.filesEditedCount ?? 0
      return count > 0 ? `${count} files` : null
    }
    case 'tokens': {
      const total = Number((session.totalInputTokens ?? 0n) + (session.totalOutputTokens ?? 0n))
      if (total <= 0) return null
      if (total >= 1_000_000) return `${(total / 1_000_000).toFixed(1)}M tokens`
      if (total >= 1_000) return `${(total / 1_000).toFixed(0)}K tokens`
      return `${total} tokens`
    }
    default:
      return null
  }
}

export function HistoryView() {
  const navigate = useNavigate()
  const { data: summaries } = useProjectSummaries()
  const projectIds = useMemo(() => (summaries ?? []).map(s => s.name), [summaries])
  const { sessions: allSessions, isLoading } = useAllSessions(projectIds)

  // URL-persisted filter/sort state
  const [searchParams, setSearchParams] = useSearchParams()
  const [filters, setFilters] = useSessionFilters(searchParams, setSearchParams)

  const [searchText, setSearchText] = useState('')
  const [selectedProjects, setSelectedProjects] = useState<Set<string>>(new Set())
  const [timeFilter, setTimeFilter] = useState<TimeFilter>('all')
  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const [showProjectFilter, setShowProjectFilter] = useState(false)
  const searchRef = useRef<HTMLInputElement>(null)
  const filterRef = useRef<HTMLDivElement>(null)

  // Detect if we arrived from a dashboard deep-link (non-default sort or filter in URL)
  const hasDeepLinkSort = filters.sort !== 'recent'
  const hasDeepLinkFilter = filters.hasCommits !== 'any' || filters.hasSkills !== 'any' || filters.highReedit !== null || filters.minDuration !== null
  const hasDeepLink = hasDeepLinkSort || hasDeepLinkFilter

  // Focus search on mount (only if not deep-linked)
  useEffect(() => {
    if (!hasDeepLink) {
      searchRef.current?.focus()
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // Close project filter on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (filterRef.current && !filterRef.current.contains(e.target as Node)) {
        setShowProjectFilter(false)
      }
    }
    if (showProjectFilter) {
      document.addEventListener('mousedown', handleClick)
      return () => document.removeEventListener('mousedown', handleClick)
    }
  }, [showProjectFilter])

  // Extract unique branches from sessions for the filter popover
  const availableBranches = useMemo(() => {
    const set = new Set<string>()
    for (const s of allSessions) {
      if (s.gitBranch) set.add(s.gitBranch)
    }
    return [...set].sort()
  }, [allSessions])

  // Map session IDs to project display names
  const projectDisplayNames = useMemo(() => {
    if (!summaries) return new Map<string, string>()
    const map = new Map<string, string>()
    for (const s of summaries) {
      map.set(s.name, s.displayName)
    }
    return map
  }, [summaries])

  // Apply filters and sorting
  const filteredSessions = useMemo(() => {
    const now = Math.floor(Date.now() / 1000)
    const cutoffs: Record<TimeFilter, number> = {
      all: 0,
      today: now - 86400,
      '7d': now - 7 * 86400,
      '30d': now - 30 * 86400,
    }
    const cutoff = cutoffs[timeFilter]
    const query = searchText.toLowerCase().trim()

    let filtered = allSessions.filter(s => {
      // Time filter
      if (cutoff > 0 && Number(s.modifiedAt) < cutoff) return false

      // Project filter
      if (selectedProjects.size > 0 && !selectedProjects.has(s.project)) return false

      // Date filter (from sparkline click)
      if (selectedDate) {
        const d = new Date(Number(s.modifiedAt) * 1000)
        const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
        if (key !== selectedDate) return false
      }

      // NEW FILTER LOGIC: Branch filter
      if (filters.branches.length > 0) {
        if (!s.gitBranch || !filters.branches.includes(s.gitBranch)) return false
      }

      // NEW FILTER LOGIC: Model filter
      if (filters.models.length > 0) {
        if (!s.primaryModel || !filters.models.includes(s.primaryModel)) return false
      }

      // NEW FILTER LOGIC: Has commits filter
      if (filters.hasCommits === 'yes' && (s.commitCount ?? 0) === 0) return false
      if (filters.hasCommits === 'no' && (s.commitCount ?? 0) > 0) return false

      // NEW FILTER LOGIC: Has skills filter
      if (filters.hasSkills === 'yes' && (s.skillsUsed ?? []).length === 0) return false
      if (filters.hasSkills === 'no' && (s.skillsUsed ?? []).length > 0) return false

      // NEW FILTER LOGIC: Minimum duration
      if (filters.minDuration !== null && (s.durationSeconds ?? 0) < filters.minDuration) return false

      // NEW FILTER LOGIC: Minimum files edited
      if (filters.minFiles !== null && (s.filesEditedCount ?? 0) < filters.minFiles) return false

      // NEW FILTER LOGIC: Minimum tokens
      if (filters.minTokens !== null) {
        const totalTokens = Number((s.totalInputTokens ?? 0n) + (s.totalOutputTokens ?? 0n))
        if (totalTokens < filters.minTokens) return false
      }

      // NEW FILTER LOGIC: High re-edit rate
      if (filters.highReedit === true) {
        const filesEdited = s.filesEditedCount ?? 0
        const reeditedFiles = s.reeditedFilesCount ?? 0
        const reeditRate = filesEdited > 0 ? reeditedFiles / filesEdited : 0
        if (reeditRate <= 0.2) return false
      }

      // Text search
      if (query) {
        const haystack = [
          s.preview,
          s.lastMessage,
          ...(s.filesTouched ?? []),
          ...(s.skillsUsed ?? []),
          s.project,
        ].join(' ').toLowerCase()
        return haystack.includes(query)
      }

      return true
    })

    // Apply sorting
    if (filters.sort !== 'recent') {
      filtered = [...filtered].sort((a, b) => {
        switch (filters.sort) {
          case 'tokens': {
            const aTokens = Number((a.totalInputTokens ?? 0n) + (a.totalOutputTokens ?? 0n))
            const bTokens = Number((b.totalInputTokens ?? 0n) + (b.totalOutputTokens ?? 0n))
            return bTokens - aTokens
          }
          case 'prompts':
            return (b.userPromptCount ?? 0) - (a.userPromptCount ?? 0)
          case 'files_edited':
            return (b.filesEditedCount ?? 0) - (a.filesEditedCount ?? 0)
          case 'duration':
            return (b.durationSeconds ?? 0) - (a.durationSeconds ?? 0)
          default:
            return 0
        }
      })
    }

    return filtered
  }, [allSessions, searchText, selectedProjects, timeFilter, selectedDate, filters])

  const isFiltered = searchText || selectedProjects.size > 0 || timeFilter !== 'all' || selectedDate || filters.sort !== 'recent' || filters.hasCommits !== 'any' || filters.hasSkills !== 'any' || filters.highReedit !== null || filters.minDuration !== null || filters.minFiles !== null || filters.minTokens !== null || filters.branches.length > 0 || filters.models.length > 0

  const tooManyToGroup = shouldDisableGrouping(filteredSessions.length);

  // Auto-reset groupBy when session count exceeds the limit
  const [groupByAutoReset, setGroupByAutoReset] = useState(false);
  useEffect(() => {
    if (tooManyToGroup && filters.groupBy !== 'none') {
      setFilters({ ...filters, groupBy: 'none' });
      setGroupByAutoReset(true);
    } else if (!tooManyToGroup) {
      setGroupByAutoReset(false);
    }
  }, [tooManyToGroup]); // eslint-disable-line react-hooks/exhaustive-deps

  // Use groupSessions if groupBy is set, otherwise fall back to date-based grouping
  const groups = useMemo(() => {
    if (filters.groupBy !== 'none' && !tooManyToGroup) {
      return groupSessions(filteredSessions, filters.groupBy)
    }
    // Default behavior: group by date when sort is 'recent', otherwise single group
    return filters.sort === 'recent' ? groupSessionsByDate(filteredSessions) : [{ label: SORT_LABELS[filters.sort], sessions: filteredSessions }]
  }, [filteredSessions, filters.groupBy, filters.sort, tooManyToGroup])

  // Collapse state for group headers
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

  const toggleGroup = useCallback((label: string) => {
    setCollapsedGroups(prev => {
      const next = new Set(prev);
      if (next.has(label)) {
        next.delete(label);
      } else {
        next.add(label);
      }
      return next;
    });
  }, []);

  // Project list sorted by session count
  const sortedProjects = useMemo(() => {
    return [...(summaries ?? [])].sort((a, b) => b.sessionCount - a.sessionCount)
  }, [summaries])

  function toggleProject(name: string) {
    setSelectedProjects(prev => {
      const next = new Set(prev)
      if (next.has(name)) {
        next.delete(name)
      } else {
        next.add(name)
      }
      return next
    })
  }

  function clearAll() {
    setSearchText('')
    setSelectedProjects(new Set())
    setTimeFilter('all')
    setSelectedDate(null)
    setFilters(DEFAULT_FILTERS)
  }

  const timeOptions: { value: TimeFilter; label: string }[] = [
    { value: 'all', label: 'All time' },
    { value: 'today', label: 'Today' },
    { value: '7d', label: '7 days' },
    { value: '30d', label: '30 days' },
  ]

  if (isLoading) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="max-w-3xl mx-auto px-6 py-5">
          <Skeleton label="sessions" rows={5} withHeader={true} />
        </div>
      </div>
    )
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-6 py-5">

        {/* Deep-link context banner */}
        {hasDeepLink && (
          <div className="mb-4 flex items-center gap-3 px-4 py-3 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg">
            <button
              onClick={() => navigate('/')}
              className="p-1 -ml-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              aria-label="Back to dashboard"
            >
              <ArrowLeft className="w-4 h-4" />
            </button>
            <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
              {hasDeepLinkSort && (
                <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 text-xs font-medium text-gray-700 dark:text-gray-300">
                  {SORT_ICONS[filters.sort]}
                  {SORT_LABELS[filters.sort]}
                </span>
              )}
              {hasDeepLinkFilter && (
                <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 text-xs font-medium text-gray-700 dark:text-gray-300">
                  <TrendingUp className="w-3.5 h-3.5" />
                  Filtered
                </span>
              )}
              <span className="text-gray-400 tabular-nums">{filteredSessions.length} sessions</span>
            </div>
            <button
              onClick={clearAll}
              className="ml-auto text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* Activity sparkline chart */}
        <ActivitySparkline
          sessions={allSessions}
          selectedDate={selectedDate}
          onDateSelect={setSelectedDate}
        />

        {/* Search + Filters bar */}
        <div className="mt-5 space-y-3">
          {/* Search input */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              ref={searchRef}
              type="text"
              value={searchText}
              onChange={e => setSearchText(e.target.value)}
              placeholder="Search sessions, files, skills..."
              className="w-full pl-9 pr-9 py-2.5 text-sm bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg outline-none transition-colors focus:bg-white dark:focus:bg-gray-900 focus:border-gray-400 dark:focus:border-gray-500 focus:ring-1 focus:ring-gray-400/20 dark:focus:ring-gray-500/20 placeholder:text-gray-400 text-gray-900 dark:text-gray-100"
            />
            {searchText && (
              <button
                onClick={() => setSearchText('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>

          {/* Filter row: session filter/sort + time + project */}
          <div className="flex items-center gap-2 flex-wrap">
            {/* NEW: SessionToolbar with view mode toggle */}
            <SessionToolbar
              filters={filters}
              onFiltersChange={setFilters}
              onClearFilters={() => setFilters(DEFAULT_FILTERS)}
              groupByDisabled={tooManyToGroup}
              branches={availableBranches}
            />

            <div className="w-px h-5 bg-gray-200 dark:bg-gray-700" />

            {/* Time filters */}
            <div className="flex items-center gap-0.5 p-0.5 bg-gray-100 dark:bg-gray-800 rounded-md">
              {timeOptions.map(opt => (
                <button
                  key={opt.value}
                  onClick={() => setTimeFilter(opt.value)}
                  className={`px-2.5 py-1 text-xs font-medium rounded-md transition-all ${
                    timeFilter === opt.value
                      ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                      : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>

            {/* Project filter dropdown */}
            <div className="relative" ref={filterRef}>
              <button
                onClick={() => setShowProjectFilter(!showProjectFilter)}
                className={`inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md border transition-all ${
                  selectedProjects.size > 0
                    ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-300'
                    : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'
                }`}
              >
                <FolderOpen className="w-3.5 h-3.5" />
                {selectedProjects.size > 0
                  ? `${selectedProjects.size} project${selectedProjects.size > 1 ? 's' : ''}`
                  : 'Projects'}
              </button>

              {showProjectFilter && (
                <div className="absolute top-full left-0 mt-1.5 w-60 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 py-1 max-h-64 overflow-y-auto">
                  {selectedProjects.size > 0 && (
                    <button
                      onClick={() => setSelectedProjects(new Set())}
                      className="w-full text-left px-3 py-1.5 text-xs text-gray-500 hover:bg-gray-50 dark:hover:bg-gray-700 border-b border-gray-100 dark:border-gray-700"
                    >
                      Clear selection
                    </button>
                  )}
                  {sortedProjects.map(p => {
                    const checked = selectedProjects.has(p.name)
                    return (
                      <button
                        key={p.name}
                        onClick={() => toggleProject(p.name)}
                        className="w-full flex items-center gap-2.5 px-3 py-2 text-left hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
                      >
                        <div className={`w-3.5 h-3.5 rounded border flex-shrink-0 flex items-center justify-center transition-colors ${
                          checked ? 'bg-blue-500 border-blue-500' : 'border-gray-300 dark:border-gray-600'
                        }`}>
                          {checked && (
                            <svg className="w-2.5 h-2.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                            </svg>
                          )}
                        </div>
                        <span className="text-sm text-gray-700 dark:text-gray-300 truncate flex-1">{p.displayName}</span>
                        <span className="text-xs text-gray-400 tabular-nums">{p.sessionCount}</span>
                      </button>
                    )
                  })}
                </div>
              )}
            </div>

            {/* Active filter summary */}
            {isFiltered && (
              <>
                <div className="h-4 w-px bg-gray-200 dark:bg-gray-700" />
                <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
                  {filteredSessions.length} of {allSessions.length}
                </span>
                <button
                  onClick={clearAll}
                  className="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 underline underline-offset-2"
                >
                  Clear all
                </button>
              </>
            )}
          </div>
        </div>

        {/* Grouping safeguard warning */}
        {tooManyToGroup && groupByAutoReset && (
          <div className="mt-3 px-3 py-2 bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800 rounded-lg text-xs text-amber-700 dark:text-amber-300">
            Grouping disabled — {filteredSessions.length} sessions exceeds the {MAX_GROUPABLE_SESSIONS} session limit. Use filters to narrow results.
          </div>
        )}

        {/* Session List or Table */}
        <div className="mt-5">
          {filteredSessions.length > 0 ? (
            filters.viewMode === 'table' ? (
              /* Table view */
              <CompactSessionTable
                sessions={filteredSessions}
                onSort={(column) => {
                  // Map table column to SessionSort
                  const sortMap: Record<SortColumn, SessionSort> = {
                    time: 'recent',
                    branch: 'recent', // No direct branch sort yet
                    prompts: 'prompts',
                    files: 'files_edited',
                    commits: 'recent', // No direct commits sort yet
                    duration: 'duration',
                  }
                  const newSort = sortMap[column] || 'recent'
                  setFilters({ ...filters, sort: newSort })
                }}
                sortColumn={
                  filters.sort === 'prompts' ? 'prompts' :
                  filters.sort === 'tokens' ? 'prompts' :
                  filters.sort === 'files_edited' ? 'files' :
                  filters.sort === 'duration' ? 'duration' :
                  'time'
                }
                sortDirection="desc"
              />
            ) : (
              /* Timeline view */
              <div>
                {groups.map(group => {
                  const isCollapsed = collapsedGroups.has(group.label);
                  return (
                    <div key={group.label}>
                      {/* Group header (collapsible) */}
                      <button
                        type="button"
                        onClick={() => toggleGroup(group.label)}
                        className="sticky top-0 z-10 w-full bg-white/95 dark:bg-gray-950/95 backdrop-blur-sm py-2 flex items-center gap-3 cursor-pointer group/header"
                        aria-expanded={!isCollapsed}
                      >
                        <ChevronDown
                          className={cn(
                            'w-3.5 h-3.5 text-gray-400 transition-transform duration-150',
                            isCollapsed && '-rotate-90'
                          )}
                        />
                        <span className="text-[13px] font-semibold text-gray-500 dark:text-gray-400 tracking-tight whitespace-nowrap group-hover/header:text-gray-700 dark:group-hover/header:text-gray-300 transition-colors">
                          {group.label}
                        </span>
                        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
                        <span className="text-[11px] text-gray-400 tabular-nums whitespace-nowrap" aria-label={`${group.sessions.length} sessions`}>
                          {group.sessions.length}
                        </span>
                      </button>

                      {/* Cards (hidden when collapsed) */}
                      {!isCollapsed && (
                        <div className="space-y-1.5 pb-3">
                          {group.sessions.map((session, idx) => {
                            const metric = filters.sort !== 'recent' ? formatSortMetric(session, filters.sort) : null
                            return (
                              <div key={session.id} className="relative">
                                {/* Rank badge for non-default sorts */}
                                {filters.sort !== 'recent' && (
                                  <div className="absolute -left-1 top-3 z-10 w-5 h-5 rounded-full bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 flex items-center justify-center">
                                    <span className="text-[10px] font-bold text-gray-500 dark:text-gray-400 tabular-nums">{idx + 1}</span>
                                  </div>
                                )}
                                <Link
                                  to={`/project/${encodeURIComponent(session.project)}/session/${sessionSlug(session.preview, session.id)}`}
                                  className="block focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2 rounded-lg"
                                >
                                  <SessionCard
                                    session={session}
                                    isSelected={false}
                                    projectDisplayName={projectDisplayNames.get(session.project)}
                                  />
                                </Link>
                                {/* Sort metric badge overlay */}
                                {metric && (
                                  <div className="absolute right-3 top-3 px-2 py-0.5 rounded-full bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 text-[11px] font-medium text-gray-500 dark:text-gray-400 tabular-nums">
                                    {metric}
                                  </div>
                                )}
                              </div>
                            )
                          })}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )
          ) : (
            <SessionsEmptyState isFiltered={isFiltered} onClearFilters={clearAll} />
          )}
        </div>
      </div>
    </div>
  )
}
