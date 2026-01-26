import type { ProjectInfo } from '../hooks/use-projects'

interface StatusBarProps {
  projects: ProjectInfo[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const totalSessions = projects.reduce((sum, p) => sum + p.sessions.length, 0)
  const latestActivity = projects[0]?.sessions[0]?.modifiedAt

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
    <footer className="h-8 bg-white border-t border-gray-200 px-4 flex items-center justify-between text-xs text-gray-500">
      <span>
        {projects.length} projects · {totalSessions} sessions
      </span>
      {latestActivity && (
        <span>
          Last activity: {formatLastActivity(latestActivity)}
        </span>
      )}
    </footer>
  )
}
