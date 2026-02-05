import { useMemo } from 'react'
import { useParams, useSearchParams } from 'react-router-dom'
import { FolderOpen } from 'lucide-react'
import { useProjectSummaries, useProjectSessions } from '../hooks/use-projects'
import { DateGroupedList } from './DateGroupedList'
import { CompactSessionTable } from './CompactSessionTable'
import type { SortColumn } from './CompactSessionTable'
import { SessionToolbar } from './SessionToolbar'
import { useSessionFilters, DEFAULT_FILTERS } from '../hooks/use-session-filters'
import type { SessionSort } from '../hooks/use-session-filters'
import { groupSessionsByDate } from '../lib/date-groups'
import { Skeleton, EmptyState, ErrorState } from './LoadingStates'

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

  // Group sessions by date for timeline view
  const groups = filteredSessions.length > 0 ? groupSessionsByDate(filteredSessions) : []

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
            />

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
                /* Timeline view */
                <div>
                  {groups.map(group => (
                    <div key={group.label}>
                      {/* Group header */}
                      <div className="sticky top-0 z-10 bg-white/95 dark:bg-gray-950/95 backdrop-blur-sm py-2 flex items-center gap-3">
                        <span className="text-[13px] font-semibold text-gray-500 dark:text-gray-400 tracking-tight whitespace-nowrap">
                          {group.label}
                        </span>
                        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
                        <span className="text-[11px] text-gray-400 tabular-nums whitespace-nowrap" aria-label={`${group.sessions.length} sessions`}>
                          {group.sessions.length}
                        </span>
                      </div>

                      {/* Cards */}
                      <DateGroupedList sessions={group.sessions} />
                    </div>
                  ))}
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
