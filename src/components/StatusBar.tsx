import type { ProjectSummary } from '../hooks/use-projects'

interface StatusBarProps {
  projects: ProjectSummary[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const totalSessions = projects.reduce((sum, p) => sum + p.sessionCount, 0)
  const latestTimestamp = projects.reduce((max, p) => {
    const t = p.lastActivityAt ? Number(p.lastActivityAt) : 0
    return t > max ? t : max
  }, 0)

  const formatLastActivity = (ts: number) => {
    const date = new Date(ts * 1000)
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
      {latestTimestamp > 0 && (
        <span>
          Last activity: {formatLastActivity(latestTimestamp)}
        </span>
      )}
    </footer>
  )
}
