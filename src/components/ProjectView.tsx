import { useParams, useSearchParams } from 'react-router-dom'
import { FolderOpen } from 'lucide-react'
import { useProjectSummaries, useProjectSessions } from '../hooks/use-projects'
import { DateGroupedList } from './DateGroupedList'
import { Skeleton, EmptyState, ErrorState } from './LoadingStates'

export function ProjectView() {
  const { projectId } = useParams()
  const [searchParams] = useSearchParams()
  const { data: summaries } = useProjectSummaries()

  const decodedProjectId = projectId ? decodeURIComponent(projectId) : null
  const project = summaries?.find(p => p.name === decodedProjectId)

  const sort = searchParams.get('sort') || 'recent'
  const branch = searchParams.get('branch') || undefined
  const includeSidechains = searchParams.get('sidechains') === 'true'

  const { data: page, isLoading, error, refetch } = useProjectSessions(decodedProjectId ?? undefined, {
    limit: 50,
    sort,
    branch,
    includeSidechains,
  })

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
          <h1 className="text-xl font-semibold text-gray-900">
            {project?.displayName ?? decodedProjectId}
          </h1>
          <p className="text-sm text-gray-500 mt-1" aria-label={`${project?.sessionCount ?? 0} sessions in this project`}>
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
        ) : page && page.sessions.length > 0 ? (
          <>
            <DateGroupedList sessions={page.sessions} />
            {page.sessions.length < page.total && (
              <div className="text-center py-6">
                <span className="px-4 py-2 text-sm text-gray-500 bg-gray-100 rounded-lg" aria-label={`Showing ${page.sessions.length} of ${page.total} sessions`}>
                  Showing {page.sessions.length} of {page.total} sessions
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
