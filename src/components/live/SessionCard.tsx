import { Link, useSearchParams } from 'react-router-dom'
import { GitBranch } from 'lucide-react'
import type { LiveSession } from '../../hooks/use-live-sessions'
import { ContextGauge } from './ContextGauge'
import { CostTooltip } from './CostTooltip'
import { cn } from '../../lib/utils'
import { buildSessionUrl } from '../../lib/url-utils'
import { cleanPreviewText } from '../../utils/get-session-title'

interface SessionCardProps {
  session: LiveSession
}

const STATUS_CONFIG = {
  streaming: { color: 'bg-green-500', label: 'Streaming', pulse: true },
  tool_use: { color: 'bg-green-500', label: 'Tool Use', pulse: false },
  waiting_for_user: { color: 'bg-amber-500', label: 'Waiting', pulse: false },
  idle: { color: 'bg-zinc-500', label: 'Idle', pulse: false },
  complete: { color: 'bg-zinc-700', label: 'Complete', pulse: false },
} as const

export function SessionCard({ session }: SessionCardProps) {
  const [searchParams] = useSearchParams()
  const statusConfig = STATUS_CONFIG[session.status] || STATUS_CONFIG.idle
  const duration = formatDuration(session.startedAt, session.lastActivityAt)

  // Title: first user message (cleaned) > project display name > project id
  const rawTitle = session.title || ''
  const title = rawTitle ? cleanPreviewText(rawTitle) : (session.projectDisplayName || session.project)
  const hasSemanticTitle = !!rawTitle

  // Show "last message" only when different from title
  const lastMsg = session.lastUserMessage ? cleanPreviewText(session.lastUserMessage) : ''
  const showLastMsg = lastMsg && lastMsg !== title

  return (
    <Link
      to={buildSessionUrl(session.id, searchParams)}
      className="block rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4 hover:bg-gray-50 dark:hover:bg-gray-800/70 transition-colors"
    >
      {/* Header: status dot + badges + cost */}
      <div className="flex items-center gap-2 mb-1">
        <span
          className={cn(
            'inline-block h-2.5 w-2.5 rounded-full flex-shrink-0',
            statusConfig.color,
            statusConfig.pulse && 'animate-pulse'
          )}
          title={statusConfig.label}
        />
        <div className="flex items-center gap-1.5 min-w-0 flex-1 overflow-hidden">
          <span
            className="inline-block px-1.5 py-0.5 text-[10px] font-medium bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded truncate max-w-[120px]"
            title={session.projectPath || session.projectDisplayName || session.project}
          >
            {session.projectDisplayName || session.project}
          </span>
          {session.gitBranch && (
            <span
              className="inline-flex items-center gap-0.5 px-1.5 py-0.5 text-[10px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 rounded max-w-[120px]"
              title={session.gitBranch}
            >
              <GitBranch className="w-2.5 h-2.5 flex-shrink-0" />
              <span className="truncate">{session.gitBranch}</span>
            </span>
          )}
        </div>
        <CostTooltip cost={session.cost} cacheStatus={session.cacheStatus}>
          <span className="text-sm font-mono text-gray-500 dark:text-gray-400 tabular-nums flex-shrink-0">
            ${session.cost.totalUsd.toFixed(2)}
          </span>
        </CostTooltip>
      </div>

      {/* Title: semantic (first user message) or project name */}
      <p className="text-sm font-medium text-gray-900 dark:text-gray-100 line-clamp-2 mb-1">
        {title}
      </p>

      {/* Last user message (if different from title) */}
      {showLastMsg && (
        <p className="text-xs text-gray-500 dark:text-gray-400 line-clamp-1 mb-1">
          <span className="text-gray-300 dark:text-gray-600 mr-1">{'->'}</span>{lastMsg}
        </p>
      )}

      {/* Current activity */}
      {session.currentActivity && (
        <div className="text-xs text-green-600 dark:text-green-400 truncate mb-2">
          {session.currentActivity}
        </div>
      )}

      {/* Context gauge */}
      <ContextGauge
        contextWindowTokens={session.contextWindowTokens}
        model={session.model}
        status={session.status}
      />

      {/* Footer: turns, duration, cost savings */}
      <div className="flex items-center gap-3 mt-2 text-xs text-gray-400 dark:text-gray-500">
        <span>{session.turnCount} turns</span>
        {duration && <span>{duration}</span>}
      </div>
    </Link>
  )
}

function formatDuration(startedAt: number | null, lastActivityAt: number): string {
  if (!startedAt || startedAt <= 0) return ''
  const seconds = lastActivityAt - startedAt
  if (seconds < 0) return ''
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = Math.floor(minutes / 60)
  const remainingMinutes = minutes % 60
  if (hours < 24) return remainingMinutes > 0 ? `${hours}h ${remainingMinutes}m` : `${hours}h`
  const days = Math.floor(hours / 24)
  const remainingHours = hours % 24
  return remainingHours > 0 ? `${days}d ${remainingHours}h` : `${days}d`
}
