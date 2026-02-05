import { useState, useMemo, useEffect, useCallback } from 'react'
import { Link, useParams, useSearchParams } from 'react-router-dom'
import { FolderOpen, ChevronDown } from 'lucide-react'
import { useProjectSummaries, useProjectSessions } from '../hooks/use-projects'
import { SessionCard } from './SessionCard'
import { CompactSessionTable } from './CompactSessionTable'
import type { SortColumn } from './CompactSessionTable'
import { SessionToolbar } from './SessionToolbar'
import { useSessionFilters, DEFAULT_FILTERS } from '../hooks/use-session-filters'
import type { SessionSort } from '../hooks/use-session-filters'
import { groupSessionsByDate } from '../lib/date-groups'
import { groupSessions, shouldDisableGrouping, MAX_GROUPABLE_SESSIONS } from '../utils/group-sessions'
import { sessionSlug } from '../lib/url-slugs'
import { Skeleton, EmptyState, ErrorState } from './LoadingStates'
import { cn } from '../lib/utils'

/** Human-readable labels for sort options */
const SORT_LABELS: Record<SessionSort, string> = {
  recent: 'Most recent',
  tokens: 'Most tokens',
  prompts: 'Most prompts',
  files_edited: 'Most files edited',
  duration: 'Longest duration',
}

export function ProjectView() {
  const { projectId } = useParams()
  const [searchParams, setSearchParams] = useSearchParams()
  const { data: summaries } = useProjectSummaries()

  const decodedProjectId = projectId ? decodeURIComponent(projectId) : null
  const project = summaries?.find(p => p.name === decodedProjectId)

  // Use the useSessionFilters hook for URL-persisted filter state
  const [filters, setFilters] = useSessionFilters(searchParams, setSearchParams)

  const includeSidechains = searchParams.get('sidechains') === 'true'

  // Derive API branch param from filters.branches (set by sidebar click or FilterPopover)
  const apiBranch = filters.branches.length > 0 ? filters.branches[0] : undefined

  const { data: page, isLoading, error, refetch } = useProjectSessions(decodedProjectId ?? undefined, {
    limit: 50,
    sort: filters.sort,
    branch: apiBranch,
    includeSidechains,
  })

  // Apply client-side filters (branch multi-select, model, commits, etc.)
  // Must be before any early returns to satisfy React hooks rules.
  const filteredSessions = useMemo(() => {
    if (!page?.sessions) return []

    return page.sessions.filter(s => {
      // Branch filter (multi-select; API only filters by first branch)
      if (filters.branches.length > 1) {
        if (!s.gitBranch || !filters.branches.includes(s.gitBranch)) return false
      }

      // Model filter
      if (filters.models.length > 0) {
        if (!s.primaryModel || !filters.models.includes(s.primaryModel)) return false
      }

      // Has commits filter
      if (filters.hasCommits === 'yes' && (s.commitCount ?? 0) === 0) return false
      if (filters.hasCommits === 'no' && (s.commitCount ?? 0) > 0) return false

      // Has skills filter
      if (filters.hasSkills === 'yes' && (s.skillsUsed ?? []).length === 0) return false
      if (filters.hasSkills === 'no' && (s.skillsUsed ?? []).length > 0) return false

      // Minimum duration
      if (filters.minDuration !== null && (s.durationSeconds ?? 0) < filters.minDuration) return false

      // Minimum files edited
      if (filters.minFiles !== null && (s.filesEditedCount ?? 0) < filters.minFiles) return false

      // Minimum tokens
      if (filters.minTokens !== null) {
        const totalTokens = Number((s.totalInputTokens ?? 0n) + (s.totalOutputTokens ?? 0n))
        if (totalTokens < filters.minTokens) return false
      }

      // High re-edit rate
      if (filters.highReedit === true) {
        const filesEdited = s.filesEditedCount ?? 0
        const reeditedFiles = s.reeditedFilesCount ?? 0
        const reeditRate = filesEdited > 0 ? reeditedFiles / filesEdited : 0
        if (reeditRate <= 0.2) return false
      }

      return true
    })
  }, [page?.sessions, filters])

  // Extract unique branches from sessions for the filter popover
  const availableBranches = useMemo(() => {
    const set = new Set<string>()
    for (const s of page?.sessions ?? []) {
      if (s.gitBranch) set.add(s.gitBranch)
    }
    return [...set].sort()
  }, [page?.sessions])

  // Grouping safeguard
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
    return filters.sort === 'recent'
      ? groupSessionsByDate(filteredSessions)
      : [{ label: SORT_LABELS[filters.sort], sessions: filteredSessions }]
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

  if (!decodedProjectId || (!project && !isLoading)) {
    return (
      <div className="h-full flex items-center justify-center">
        <EmptyState
          icon={<FolderOpen className="w-6 h-6 text-gray-400" />}
          title="Project not found"
          description="This project may have been deleted or moved."
        />
      </div>
    )
  }

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto">
        <div className="mb-6">
          <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
            {project?.displayName ?? decodedProjectId}
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1" aria-label={`${project?.sessionCount ?? 0} sessions in this project`}>
            {project?.sessionCount ?? 0} sessions
          </p>
        </div>

        {isLoading ? (
          <Skeleton label="project sessions" rows={4} withHeader={false} />
        ) : error ? (
          <ErrorState
            message={error.message}
            onRetry={() => refetch()}
          />
        ) : page && filteredSessions.length > 0 ? (
          <>
            {/* SessionToolbar with view mode toggle */}
            <SessionToolbar
              filters={filters}
              onFiltersChange={setFilters}
              onClearFilters={() => setFilters(DEFAULT_FILTERS)}
              groupByDisabled={tooManyToGroup}
              branches={availableBranches}
            />

            {/* Grouping safeguard warning */}
            {tooManyToGroup && groupByAutoReset && (
              <div className="mt-3 px-3 py-2 bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800 rounded-lg text-xs text-amber-700 dark:text-amber-300">
                Grouping disabled — {filteredSessions.length} sessions exceeds the {MAX_GROUPABLE_SESSIONS} session limit. Use filters to narrow results.
              </div>
            )}

            {/* Session List or Table */}
            <div className="mt-5">
              {filters.viewMode === 'table' ? (
                /* Table view */
                <CompactSessionTable
                  sessions={filteredSessions}
                  onSort={(column) => {
                    // Map table column to SessionSort
                    const sortMap: Record<SortColumn, SessionSort> = {
                      time: 'recent',
                      branch: 'recent', // No direct branch sort yet
                      prompts: 'prompts',
                      tokens: 'tokens',
                      files: 'files_edited',
                      loc: 'recent', // No direct LOC sort yet
                      commits: 'recent', // No direct commits sort yet
                      duration: 'duration',
                    }
                    const newSort = sortMap[column] || 'recent'
                    setFilters({ ...filters, sort: newSort })
                  }}
                  sortColumn={
                    filters.sort === 'prompts' ? 'prompts' :
                    filters.sort === 'tokens' ? 'tokens' :
                    filters.sort === 'files_edited' ? 'files' :
                    filters.sort === 'duration' ? 'duration' :
                    'time'
                  }
                  sortDirection="desc"
                />
              ) : (
                /* Timeline view with collapsible group headers */
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
                            {group.sessions.map((session) => (
                              <Link
                                key={session.id}
                                to={`/project/${encodeURIComponent(session.project)}/session/${sessionSlug(session.preview, session.id)}`}
                                className="block focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2 rounded-lg"
                              >
                                <SessionCard
                                  session={session}
                                  isSelected={false}
                                />
                              </Link>
                            ))}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {filteredSessions.length < page.total && (
              <div className="text-center py-6">
                <span className="px-4 py-2 text-sm text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800 rounded-lg" aria-label={`Showing ${filteredSessions.length} of ${page.total} sessions`}>
                  Showing {filteredSessions.length} of {page.total} sessions
                </span>
              </div>
            )}
          </>
        ) : (
          <EmptyState
            icon={<FolderOpen className="w-6 h-6 text-gray-400" />}
            title="No sessions yet"
            description="Sessions will appear here after you use Claude Code in this project."
          />
        )}
      </div>
    </div>
  )
}
