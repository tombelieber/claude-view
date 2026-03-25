import { Clock, Crown, MessageSquare, Users } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { TeamSummary } from '../../types/generated'

interface TeamCardProps {
  team: TeamSummary
  onClick: () => void
}

function formatDuration(secs: number | null | undefined): string {
  if (!secs) return '\u2014'
  if (secs < 60) return `${secs}s`
  return `${Math.round(secs / 60)} min`
}

function formatDate(ms: number): string {
  if (ms <= 0) return '—'
  const d = new Date(ms)
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

export function TeamCard({ team, onClick }: TeamCardProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'w-full text-left p-4 rounded-lg border transition-colors',
        'bg-white dark:bg-gray-900',
        'border-gray-200 dark:border-gray-800',
        'hover:border-blue-300 dark:hover:border-blue-700',
        'hover:shadow-sm',
        'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
      )}
    >
      <div className="flex items-start justify-between mb-2">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 truncate">
          {team.name}
        </h3>
        <span className="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap ml-2">
          {formatDate(team.createdAt)}
        </span>
      </div>

      <p className="text-xs text-gray-500 dark:text-gray-400 line-clamp-2 mb-3">
        {team.description}
      </p>

      <div className="flex items-center gap-1.5 mb-2">
        <Crown className="w-3 h-3 text-yellow-500" />
        <Users className="w-3 h-3 text-gray-400" />
        <span className="text-xs text-gray-500 dark:text-gray-400">{team.memberCount} members</span>
      </div>

      <div className="flex items-center gap-3 text-xs text-gray-400 dark:text-gray-500">
        <span className="flex items-center gap-1">
          <MessageSquare className="w-3 h-3" />
          {team.messageCount} msgs
        </span>
        <span className="flex items-center gap-1">
          <Clock className="w-3 h-3" />
          {formatDuration(team.durationEstimateSecs)}
        </span>
        <span className="ml-auto text-xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800">
          {team.models.join(' + ')}
        </span>
      </div>
    </button>
  )
}
