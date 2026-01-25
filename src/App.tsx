import { useState, useMemo, useEffect, useCallback } from 'react'
import { Loader2, FolderOpen, HelpCircle, Settings, Search } from 'lucide-react'
import { useProjects, type ProjectInfo, type SessionInfo } from './hooks/use-projects'
import { ConversationView } from './components/ConversationView'
import { SessionCard } from './components/SessionCard'
import { CommandPalette } from './components/CommandPalette'
import { parseQuery, filterSessions } from './lib/search'
import { cn } from './lib/utils'

interface SelectedSession {
  projectDir: string
  sessionId: string
}

function Sidebar({
  projects,
  selectedProject,
  onProjectClick,
  onFilterClick,
}: {
  projects: ProjectInfo[]
  selectedProject: string | null
  onProjectClick: (project: ProjectInfo) => void
  onFilterClick: (query: string) => void
}) {
  const selectedProjectData = projects.find(p => p.name === selectedProject)

  // Calculate per-project stats
  const projectStats = useMemo(() => {
    if (!selectedProjectData) return null

    const skillCounts = new Map<string, number>()
    const fileCounts = new Map<string, number>()

    for (const session of selectedProjectData.sessions) {
      for (const skill of session.skillsUsed ?? []) {
        skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
      }
      for (const file of session.filesTouched ?? []) {
        fileCounts.set(file, (fileCounts.get(file) || 0) + 1)
      }
    }

    return {
      topSkills: Array.from(skillCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3),
      topFiles: Array.from(fileCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3),
    }
  }, [selectedProjectData])

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

      {/* Per-project stats */}
      {projectStats && (
        <div className="border-t border-gray-200 p-3 space-y-3">
          {projectStats.topSkills.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-1.5">
                Skills
              </p>
              <div className="flex flex-wrap gap-1">
                {projectStats.topSkills.map(([skill, count]) => (
                  <button
                    key={skill}
                    onClick={() => onFilterClick(`project:${selectedProjectData!.displayName} skill:${skill.replace('/', '')}`)}
                    className="px-1.5 py-0.5 text-[11px] font-mono bg-gray-200 hover:bg-gray-300 text-gray-600 rounded transition-colors"
                  >
                    {skill} <span className="text-gray-400">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {projectStats.topFiles.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-1.5">
                Top Files
              </p>
              <div className="space-y-0.5">
                {projectStats.topFiles.map(([file, count]) => (
                  <button
                    key={file}
                    onClick={() => onFilterClick(`project:${selectedProjectData!.displayName} path:${file}`)}
                    className="w-full flex items-center justify-between px-1.5 py-0.5 text-[11px] hover:bg-gray-200 rounded transition-colors"
                  >
                    <span className="truncate text-gray-600">{file}</span>
                    <span className="text-gray-400 tabular-nums">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
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

  // Format date for display
  const formatLastActivity = (dateString: string) => {
    const date = new Date(dateString)
    return date.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    })
  }

  return (
    <footer className="h-8 bg-white border-t border-gray-200 px-4 flex items-center text-xs text-gray-500">
      <span>
        {projects.length} projects &bull; {totalSessions} sessions
        {latestActivity && (
          <> &bull; Last activity: {formatLastActivity(latestActivity)}</>
        )}
      </span>
    </footer>
  )
}

function SearchResults({
  sessions,
  query,
  onSessionClick,
  onClearSearch,
}: {
  sessions: SessionInfo[]
  query: string
  onSessionClick: (session: SessionInfo) => void
  onClearSearch: () => void
}) {
  return (
    <main className="flex-1 overflow-y-auto bg-gray-50 p-6">
      <div className="max-w-3xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-xl font-semibold text-gray-900">
              Search Results
            </h1>
            <p className="text-sm text-gray-500 mt-1">
              {sessions.length} sessions matching "{query}"
            </p>
          </div>
          <button
            onClick={onClearSearch}
            className="px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors"
          >
            Clear search
          </button>
        </div>

        <div className="space-y-3">
          {sessions.map((session) => (
            <SessionCard
              key={session.id}
              session={session}
              isSelected={false}
              isActive={false}
              onClick={() => onSessionClick(session)}
            />
          ))}
        </div>

        {sessions.length === 0 && (
          <div className="text-center py-12 text-gray-500">
            <p>No sessions match your search.</p>
            <p className="text-sm mt-1">Try different keywords or filters.</p>
          </div>
        )}
      </div>
    </main>
  )
}

export default function App() {
  const { data: projects, isLoading, error } = useProjects()
  const [selectedProjectName, setSelectedProjectName] = useState<string | null>(null)
  const [selectedSession, setSelectedSession] = useState<SelectedSession | null>(null)
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [recentSearches, setRecentSearches] = useState<string[]>(() => {
    const saved = localStorage.getItem('claude-view-recent-searches')
    return saved ? JSON.parse(saved) : []
  })

  const selectedProject = useMemo(() => {
    if (!projects || !selectedProjectName) return null
    return projects.find(p => p.name === selectedProjectName) || null
  }, [projects, selectedProjectName])

  // Parse URL on initial load
  useEffect(() => {
    const path = window.location.pathname
    const match = path.match(/^\/session\/([^/]+)\/(.+)$/)
    if (match) {
      const [, projectDir, sessionId] = match
      setSelectedSession({ projectDir: decodeURIComponent(projectDir), sessionId })
      // Find and select the project
      if (projects) {
        const project = projects.find(p =>
          p.sessions.some(s => s.project === decodeURIComponent(projectDir) && s.id === sessionId)
        )
        if (project) setSelectedProjectName(project.name)
      }
    }
  }, [projects])

  // Handle browser back/forward
  useEffect(() => {
    const handlePopState = (event: PopStateEvent) => {
      if (event.state?.session) {
        setSelectedSession(event.state.session)
        if (event.state.projectName) {
          setSelectedProjectName(event.state.projectName)
        }
      } else {
        setSelectedSession(null)
      }
    }

    window.addEventListener('popstate', handlePopState)
    return () => window.removeEventListener('popstate', handlePopState)
  }, [])

  // Cmd+K to open search
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setIsSearchOpen(true)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const handleSearch = useCallback((query: string) => {
    setSearchQuery(query)
    setIsSearchOpen(false)

    // Save to recent searches
    setRecentSearches(prev => {
      const updated = [query, ...prev.filter(s => s !== query)].slice(0, 10)
      localStorage.setItem('claude-view-recent-searches', JSON.stringify(updated))
      return updated
    })
  }, [])

  const filteredSessions = useMemo(() => {
    if (!projects || !searchQuery) return null

    const allSessions = projects.flatMap(p => p.sessions)
    const parsed = parseQuery(searchQuery)
    return filterSessions(allSessions, projects, parsed)
  }, [projects, searchQuery])

  const handleProjectClick = (project: ProjectInfo) => {
    setSelectedProjectName(project.name)
    setSelectedSession(null)
    // Update URL to root when selecting project
    window.history.pushState({ projectName: project.name }, '', '/')
  }

  const handleSessionClick = useCallback((session: SessionInfo) => {
    const newSession = {
      projectDir: session.project,
      sessionId: session.id,
    }
    setSelectedSession(newSession)
    // Push to history with session URL
    const url = `/session/${encodeURIComponent(session.project)}/${session.id}`
    window.history.pushState(
      { session: newSession, projectName: selectedProjectName },
      '',
      url
    )
  }, [selectedProjectName])

  const handleBackFromConversation = useCallback(() => {
    setSelectedSession(null)
    // Go back in history (this triggers popstate)
    window.history.back()
  }, [])

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
          {/* Search button */}
          <button
            onClick={() => setIsSearchOpen(true)}
            className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-500 hover:text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
          >
            <Search className="w-4 h-4" />
            <span className="hidden sm:inline">Search</span>
            <kbd className="hidden sm:inline text-xs text-gray-400">Cmd+K</kbd>
          </button>
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
          onFilterClick={(query) => setSearchQuery(query)}
        />
        {selectedSession ? (
          <ConversationView
            projectDir={selectedSession.projectDir}
            projectName={selectedProjectName || selectedSession.projectDir}
            sessionId={selectedSession.sessionId}
            onBack={handleBackFromConversation}
          />
        ) : searchQuery && filteredSessions ? (
          <SearchResults
            sessions={filteredSessions}
            query={searchQuery}
            onSessionClick={handleSessionClick}
            onClearSearch={() => setSearchQuery('')}
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

      {/* Command Palette */}
      <CommandPalette
        isOpen={isSearchOpen}
        onClose={() => setIsSearchOpen(false)}
        onSearch={handleSearch}
        recentSearches={recentSearches}
      />
    </div>
  )
}
