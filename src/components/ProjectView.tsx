import { useParams, useSearchParams } from 'react-router-dom'
import { Loader2 } from 'lucide-react'
import { useProjectSummaries, useProjectSessions } from '../hooks/use-projects'
import { DateGroupedList } from './DateGroupedList'

export function ProjectView() {
  const { projectId } = useParams()
  const [searchParams] = useSearchParams()
  const { data: summaries } = useProjectSummaries()

  const decodedProjectId = projectId ? decodeURIComponent(projectId) : null
  const project = summaries?.find(p => p.name === decodedProjectId)

  const sort = searchParams.get('sort') || 'recent'
  const branch = searchParams.get('branch') || undefined
  const includeSidechains = searchParams.get('sidechains') === 'true'

  const { data: page, isLoading } = useProjectSessions(decodedProjectId ?? undefined, {
    limit: 50,
    sort,
    branch,
    includeSidechains,
  })

  if (!decodedProjectId || (!project && !isLoading)) {
    return (
      <div className="p-6">
        <p className="text-gray-500">Project not found</p>
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
          <p className="text-sm text-gray-500 mt-1">
            {project?.sessionCount ?? 0} sessions
          </p>
        </div>

        {isLoading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="w-5 h-5 animate-spin text-gray-400" />
          </div>
        ) : page ? (
          <>
            <DateGroupedList sessions={page.sessions} />
            {page.sessions.length < page.total && (
              <div className="text-center py-6">
                <span className="px-4 py-2 text-sm text-gray-500 bg-gray-100 rounded-lg">
                  Showing {page.sessions.length} of {page.total} sessions
                </span>
              </div>
            )}
          </>
        ) : null}
      </div>
    </div>
  )
}
