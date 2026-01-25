import { useState, useMemo } from 'react'
import { ChevronDown, ChevronRight, Loader2, FolderOpen, HelpCircle, Settings } from 'lucide-react'
import { useProjects, type ProjectInfo, type SessionInfo } from './hooks/use-projects'
import { cn } from './lib/utils'

interface SelectedSession {
  projectDir: string
  sessionId: string
}

interface ProjectTreeItemProps {
  project: ProjectInfo
  isExpanded: boolean
  isSelected: boolean
  onToggle: () => void
  onClick: () => void
}

function ProjectTreeItem({ project, isExpanded, isSelected, onToggle, onClick }: ProjectTreeItemProps) {
  // Extract just the last part of the path for display
  const displayName = project.name.split('/').pop() || project.name

  return (
    <div>
      <button
        onClick={() => {
          onClick()
          onToggle()
        }}
        className={cn(
          'w-full flex items-center gap-2 px-3 py-2 text-sm text-left hover:bg-gray-200 rounded-md transition-colors',
          isSelected && 'bg-blue-50 text-blue-700'
        )}
      >
        {isExpanded ? (
          <ChevronDown className="w-4 h-4 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-4 h-4 text-gray-400 flex-shrink-0" />
        )}
        <span className="truncate flex-1">{displayName}</span>
        <span className="text-gray-400 text-xs">({project.sessions.length})</span>
      </button>
    </div>
  )
}

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / (1000 * 60))
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60))
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

  // Format time
  const timeStr = date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })

  if (diffDays === 0) {
    if (diffHours === 0 && diffMins < 60) {
      return `Today, ${timeStr}`
    }
    return `Today, ${timeStr}`
  } else if (diffDays === 1) {
    return `Yesterday, ${timeStr}`
  } else if (diffDays < 7) {
    const dayName = date.toLocaleDateString('en-US', { weekday: 'long' })
    return `${dayName}, ${timeStr}`
  } else {
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    }) + `, ${timeStr}`
  }
}

interface SessionCardProps {
  session: SessionInfo
  isSelected: boolean
  isActive: boolean
  onClick: () => void
}

function SessionCard({ session, isSelected, isActive, onClick }: SessionCardProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full text-left p-4 rounded-lg border transition-colors',
        isSelected
          ? 'bg-blue-50 border-blue-500'
          : 'bg-white border-gray-200 hover:bg-gray-50 hover:border-gray-300'
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <p className="text-sm text-gray-900 line-clamp-2 flex-1">
          "{session.preview}"
        </p>
        {isActive && (
          <span className="flex items-center gap-1 text-xs text-green-600 flex-shrink-0">
            <span className="w-2 h-2 bg-green-500 rounded-full" />
            Active
          </span>
        )}
      </div>
      <p className="text-xs text-gray-500 mt-2">
        {formatRelativeTime(session.modifiedAt)}
      </p>
    </button>
  )
}

function Sidebar({
  projects,
  selectedProject,
  expandedProjects,
  onProjectClick,
  onToggleProject,
}: {
  projects: ProjectInfo[]
  selectedProject: string | null
  expandedProjects: Set<string>
  onProjectClick: (project: ProjectInfo) => void
  onToggleProject: (projectName: string) => void
}) {
  // Group projects by their parent folder (first part of path)
  const groupedProjects = useMemo(() => {
    const groups: Record<string, ProjectInfo[]> = {}

    for (const project of projects) {
      const parts = project.name.split('/')
      // Use the parent folder as the group key, or 'Other' for single-level paths
      const groupKey = parts.length > 1 ? parts.slice(0, -1).join('/') : 'Other'
      if (!groups[groupKey]) {
        groups[groupKey] = []
      }
      groups[groupKey].push(project)
    }

    return groups
  }, [projects])

  // Recent sessions (last 5 unique projects by most recent activity)
  const recentSessions = useMemo(() => {
    return projects.slice(0, 5).map(p => ({
      project: p,
      latestSession: p.sessions[0],
    }))
  }, [projects])

  return (
    <aside className="w-64 lg:w-72 bg-gray-100 border-r border-gray-200 flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto p-4">
        {/* Projects Section */}
        <div className="mb-6">
          <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-3">
            Projects
          </h2>
          <div className="space-y-1">
            {Object.entries(groupedProjects).map(([group, groupProjects]) => (
              <div key={group}>
                {group !== 'Other' && (
                  <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500">
                    <FolderOpen className="w-3 h-3" />
                    <span className="truncate">{group}</span>
                  </div>
                )}
                <div className={cn(group !== 'Other' && 'ml-2')}>
                  {groupProjects.map((project) => (
                    <ProjectTreeItem
                      key={project.name}
                      project={project}
                      isExpanded={expandedProjects.has(project.name)}
                      isSelected={selectedProject === project.name}
                      onToggle={() => onToggleProject(project.name)}
                      onClick={() => onProjectClick(project)}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Recent Section */}
        <div>
          <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-3">
            Recent
          </h2>
          <div className="space-y-2">
            {recentSessions.map(({ project, latestSession }) => (
              <button
                key={project.name}
                onClick={() => onProjectClick(project)}
                className="w-full text-left px-3 py-2 text-sm hover:bg-gray-200 rounded-md transition-colors"
              >
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-full bg-gray-300" />
                  <span className="truncate flex-1">
                    {project.name.split('/').pop()}
                  </span>
                </div>
                <p className="text-xs text-gray-400 ml-4 mt-0.5">
                  {new Date(latestSession.modifiedAt).toLocaleTimeString('en-US', {
                    hour: 'numeric',
                    minute: '2-digit',
                    hour12: true,
                  })}
                </p>
              </button>
            ))}
          </div>
        </div>
      </div>
    </aside>
  )
}

function MainContent({
  selectedProject,
  selectedSession,
  onSessionClick,
}: {
  selectedProject: ProjectInfo | null
  selectedSession: SelectedSession | null
  onSessionClick: (session: SessionInfo) => void
}) {
  if (!selectedProject) {
    return (
      <main className="flex-1 flex items-center justify-center bg-gray-50">
        <div className="text-center text-gray-500">
          <FolderOpen className="w-12 h-12 mx-auto mb-4 text-gray-300" />
          <p>Select a project to view sessions</p>
        </div>
      </main>
    )
  }

  // Find the most recent session to mark as active
  const activeSessionId = selectedProject.sessions[0]?.id

  return (
    <main className="flex-1 overflow-y-auto bg-gray-50 p-6">
      <div className="max-w-3xl mx-auto">
        <h1 className="text-xl font-semibold text-gray-900 mb-1">
          {selectedProject.name}
        </h1>
        <div className="h-0.5 bg-gray-300 mb-6" />

        <div className="space-y-3">
          {selectedProject.sessions.map((session) => (
            <SessionCard
              key={session.id}
              session={session}
              isSelected={selectedSession?.sessionId === session.id}
              isActive={session.id === activeSessionId}
              onClick={() => onSessionClick(session)}
            />
          ))}
        </div>

        {selectedProject.sessions.length >= 10 && (
          <button className="w-full mt-4 py-3 text-sm text-gray-500 hover:text-gray-700 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors">
            Load more sessions...
          </button>
        )}
      </div>
    </main>
  )
}

function StatusBar({ projects }: { projects: ProjectInfo[] }) {
  const totalSessions = projects.reduce((sum, p) => sum + p.sessions.length, 0)
  const latestActivity = projects[0]?.sessions[0]?.modifiedAt

  return (
    <footer className="h-8 bg-white border-t border-gray-200 px-4 flex items-center text-xs text-gray-500">
      <span>
        {projects.length} projects &bull; {totalSessions} sessions
        {latestActivity && (
          <> &bull; Last activity: {formatRelativeTime(latestActivity)}</>
        )}
      </span>
    </footer>
  )
}

export default function App() {
  const { data: projects, isLoading, error } = useProjects()
  const [selectedProjectName, setSelectedProjectName] = useState<string | null>(null)
  const [selectedSession, setSelectedSession] = useState<SelectedSession | null>(null)
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set())

  const selectedProject = useMemo(() => {
    if (!projects || !selectedProjectName) return null
    return projects.find(p => p.name === selectedProjectName) || null
  }, [projects, selectedProjectName])

  const handleProjectClick = (project: ProjectInfo) => {
    setSelectedProjectName(project.name)
    // Auto-expand when clicking
    setExpandedProjects(prev => {
      const next = new Set(prev)
      next.add(project.name)
      return next
    })
  }

  const handleToggleProject = (projectName: string) => {
    setExpandedProjects(prev => {
      const next = new Set(prev)
      if (next.has(projectName)) {
        next.delete(projectName)
      } else {
        next.add(projectName)
      }
      return next
    })
  }

  const handleSessionClick = (session: SessionInfo) => {
    setSelectedSession({
      projectDir: session.project,
      sessionId: session.id,
    })
  }

  if (isLoading) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="flex items-center gap-3 text-gray-600">
          <Loader2 className="w-5 h-5 animate-spin" />
          <span>Loading sessions...</span>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-center text-red-600">
          <p className="font-medium">Failed to load projects</p>
          <p className="text-sm mt-1">{error.message}</p>
        </div>
      </div>
    )
  }

  if (!projects || projects.length === 0) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-center text-gray-500">
          <FolderOpen className="w-12 h-12 mx-auto mb-4 text-gray-300" />
          <p className="font-medium">No Claude Code sessions found</p>
          <p className="text-sm mt-1">
            Start using Claude Code to see your sessions here
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="h-screen flex flex-col">
      {/* Header */}
      <header className="h-12 bg-white border-b border-gray-200 flex items-center justify-between px-4">
        <h1 className="text-lg font-semibold text-gray-900">Claude View</h1>
        <div className="flex items-center gap-2">
          <button className="p-2 text-gray-400 hover:text-gray-600 transition-colors">
            <HelpCircle className="w-5 h-5" />
          </button>
          <button className="p-2 text-gray-400 hover:text-gray-600 transition-colors">
            <Settings className="w-5 h-5" />
          </button>
        </div>
      </header>

      {/* Main Layout */}
      <div className="flex-1 flex overflow-hidden">
        <Sidebar
          projects={projects}
          selectedProject={selectedProjectName}
          expandedProjects={expandedProjects}
          onProjectClick={handleProjectClick}
          onToggleProject={handleToggleProject}
        />
        <MainContent
          selectedProject={selectedProject}
          selectedSession={selectedSession}
          onSessionClick={handleSessionClick}
        />
      </div>

      {/* Status Bar */}
      <StatusBar projects={projects} />
    </div>
  )
}
