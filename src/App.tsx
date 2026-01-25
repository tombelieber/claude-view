import { useState, useMemo } from 'react'
import { Loader2, FolderOpen, HelpCircle, Settings } from 'lucide-react'
import { useProjects, type ProjectInfo, type SessionInfo } from './hooks/use-projects'
import { ConversationView } from './components/ConversationView'
import { cn } from './lib/utils'

interface SelectedSession {
  projectDir: string
  sessionId: string
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
  onProjectClick,
}: {
  projects: ProjectInfo[]
  selectedProject: string | null
  onProjectClick: (project: ProjectInfo) => void
}) {
  return (
    <aside className="w-72 bg-gray-50/80 border-r border-gray-200 flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto py-2">
        {/* All Projects - Finder style list with path subtext */}
        {projects.map((project) => {
          const isSelected = selectedProject === project.name
          // Show parent path without the project name itself
          const parentPath = project.name.split('/').slice(0, -1).join('/')
          const hasActive = project.activeCount > 0

          return (
            <button
              key={project.name}
              onClick={() => onProjectClick(project)}
              className={cn(
                'w-full flex items-start gap-2.5 px-3 py-2 text-left transition-colors',
                isSelected
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-700 hover:bg-gray-200/70'
              )}
            >
              <FolderOpen className={cn(
                'w-4 h-4 flex-shrink-0 mt-0.5',
                isSelected ? 'text-white' : 'text-blue-400'
              )} />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="truncate font-medium text-[13px]">
                    {project.displayName}
                  </span>
                </div>
                {parentPath && (
                  <p className={cn(
                    'text-[11px] truncate mt-0.5',
                    isSelected ? 'text-blue-100' : 'text-gray-400'
                  )}>
                    {parentPath}
                  </p>
                )}
              </div>
              <div className="flex items-center gap-1.5 flex-shrink-0">
                {hasActive && (
                  <span className="flex items-center gap-1">
                    <span className={cn(
                      'w-1.5 h-1.5 rounded-full',
                      isSelected ? 'bg-green-300' : 'bg-green-500'
                    )} />
                    <span className={cn(
                      'text-xs tabular-nums',
                      isSelected ? 'text-green-200' : 'text-green-600'
                    )}>
                      {project.activeCount}
                    </span>
                  </span>
                )}
                <span className={cn(
                  'text-xs tabular-nums',
                  isSelected ? 'text-blue-100' : 'text-gray-400'
                )}>
                  {project.sessions.length}
                </span>
              </div>
            </button>
          )
        })}
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

  const selectedProject = useMemo(() => {
    if (!projects || !selectedProjectName) return null
    return projects.find(p => p.name === selectedProjectName) || null
  }, [projects, selectedProjectName])

  const handleProjectClick = (project: ProjectInfo) => {
    setSelectedProjectName(project.name)
    setSelectedSession(null) // Clear session when switching projects
  }

  const handleSessionClick = (session: SessionInfo) => {
    setSelectedSession({
      projectDir: session.project,
      sessionId: session.id,
    })
  }

  const handleBackFromConversation = () => {
    setSelectedSession(null)
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
          onProjectClick={handleProjectClick}
        />
        {selectedSession ? (
          <ConversationView
            projectDir={selectedSession.projectDir}
            projectName={selectedProjectName || selectedSession.projectDir}
            sessionId={selectedSession.sessionId}
            onBack={handleBackFromConversation}
          />
        ) : (
          <MainContent
            selectedProject={selectedProject}
            selectedSession={selectedSession}
            onSessionClick={handleSessionClick}
          />
        )}
      </div>

      {/* Status Bar */}
      <StatusBar projects={projects} />
    </div>
  )
}
