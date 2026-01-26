import { useParams, useOutletContext, Link } from 'react-router-dom'
import type { ProjectInfo } from '../hooks/use-projects'
import { SessionCard } from './SessionCard'

interface OutletContext {
  projects: ProjectInfo[]
}

export function ProjectView() {
  const { projectId } = useParams()
  const { projects } = useOutletContext<OutletContext>()

  const decodedProjectId = projectId ? decodeURIComponent(projectId) : null
  const project = projects.find(p => p.name === decodedProjectId)

  if (!project) {
    return (
      <div className="p-6">
        <p className="text-gray-500">Project not found</p>
      </div>
    )
  }

  const activeSessionId = project.sessions[0]?.id

  return (
    <div className="p-6 max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-xl font-semibold text-gray-900">
          {project.displayName}
        </h1>
        <p className="text-sm text-gray-500 mt-1">
          {project.sessions.length} sessions
          {project.activeCount > 0 && (
            <span className="text-green-600 ml-2">
              · {project.activeCount} active
            </span>
          )}
        </p>
      </div>

      <div className="space-y-3">
        {project.sessions.map((session) => (
          <Link
            key={session.id}
            to={`/session/${encodeURIComponent(session.project)}/${session.id}`}
          >
            <SessionCard
              session={session}
              isSelected={false}
              isActive={session.id === activeSessionId}
              onClick={() => {}}
            />
          </Link>
        ))}
      </div>

      {project.sessions.length >= 20 && (
        <button className="w-full mt-4 py-3 text-sm text-gray-500 hover:text-gray-700 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors">
          Load more sessions...
        </button>
      )}
    </div>
  )
}
