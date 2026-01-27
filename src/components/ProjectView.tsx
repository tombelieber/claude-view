import { useParams, useOutletContext } from 'react-router-dom'
import type { ProjectInfo } from '../hooks/use-projects'
import { DateGroupedList } from './DateGroupedList'

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

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto">
      <div className="mb-6">
        <h1 className="text-xl font-semibold text-gray-900">
          {project.displayName}
        </h1>
        <p className="text-sm text-gray-500 mt-1">
          {project.sessions.length} sessions
        </p>
      </div>

      <DateGroupedList sessions={project.sessions} />
      </div>
    </div>
  )
}
