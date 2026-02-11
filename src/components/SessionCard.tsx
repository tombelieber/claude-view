import { Terminal, Pencil, Eye, MessageSquare, GitCommit, GitBranch, FileEdit, Code2 } from 'lucide-react'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'
import type { SessionInfo } from '../hooks/use-projects'
import { WorkTypeBadge } from './WorkTypeBadge'
import { getSessionTitle, cleanPreviewText } from '../utils/get-session-title'

/**
 * Extended session info with optional Theme 3 contribution fields.
 * These fields are populated after deep indexing with contribution metrics.
 */
interface ExtendedSessionInfo extends SessionInfo {
  workType?: string | null
  aiLinesAdded?: bigint | null
  aiLinesRemoved?: bigint | null
}

interface SessionCardProps {
  session: ExtendedSessionInfo | null | undefined
  isSelected?: boolean
  projectDisplayName?: string | null
}


/**
 * Format time for session card display.
 * Returns format like "2:30 PM" for use in time ranges.
 */
function formatTimeOnly(timestamp: number): string {
  if (timestamp <= 0) return '--'
  const date = new Date(timestamp * 1000)
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

/**
 * Format the time range prefix based on date.
 * - Today: "Today"
 * - Yesterday: "Yesterday"
 * - Older: "Jan 26"
 */
function formatDatePrefix(timestamp: number): string {
  if (timestamp <= 0) return '--'
  const date = new Date(timestamp * 1000)
  const now = new Date()

  // Reset to start of day for comparison
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const targetDay = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  const diffDays = Math.floor((today.getTime() - targetDay.getTime()) / (1000 * 60 * 60 * 24))

  if (diffDays === 0) {
    return 'Today'
  } else if (diffDays === 1) {
    return 'Yesterday'
  } else {
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    })
  }
}

/**
 * Format session time range with start and end times.
 * Examples:
 * - Today 2:30 PM -> 3:15 PM
 * - Yesterday 4:00 PM -> 4:18 PM
 * - Jan 26 9:30 AM -> 12:48 PM
 */
function formatTimeRange(startTimestamp: number, endTimestamp: number): string {
  const prefix = formatDatePrefix(startTimestamp)
  const startTime = formatTimeOnly(startTimestamp)
  const endTime = formatTimeOnly(endTimestamp)
  return `${prefix} ${startTime} -> ${endTime}`
}

/**
 * Format duration in human-readable form.
 * Examples: "45 min", "2.1 hr", "15 min"
 */
function formatDuration(durationSeconds: number): string {
  if (durationSeconds < 60) {
    return `${durationSeconds}s`
  }
  const minutes = Math.round(durationSeconds / 60)
  if (minutes < 60) {
    return `${minutes} min`
  }
  const hours = durationSeconds / 3600
  return `${hours.toFixed(1)} hr`
}

/**
 * Legacy format for backward compatibility with existing components.
 */
function formatRelativeTime(timestamp: number): string {
  if (timestamp <= 0) return '--'
  // timestamp is Unix seconds, convert to milliseconds for JavaScript Date
  const date = new Date(timestamp * 1000)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

  const timeStr = date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })

  if (diffDays === 0) {
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
    }) + `, ${timeStr}`
  }
}

export function SessionCard({ session, isSelected = false, projectDisplayName }: SessionCardProps) {
  // Null safety: handle null/undefined session
  if (!session) {
    return (
      <article className="w-full text-left p-3.5 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900">
        <div className="text-sm text-gray-500 dark:text-gray-400">Session data unavailable</div>
      </article>
    )
  }

  const toolCounts = session?.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
  const editCount = (toolCounts?.edit ?? 0) + (toolCounts?.write ?? 0) // Combined edit + write
  const totalTools = editCount + (toolCounts?.bash ?? 0) + (toolCounts?.read ?? 0)

  // Clean up preview text: cascade through preview -> summary -> 'Untitled session'
  const cleanPreview = getSessionTitle(session?.preview, session?.summary)
  const cleanLast = session?.lastMessage ? cleanPreviewText(session.lastMessage) : ''

  const projectLabel = projectDisplayName || undefined

  // Calculate total tokens (input + output)
  const totalTokens = ((session?.totalInputTokens ?? 0n) as bigint) + ((session?.totalOutputTokens ?? 0n) as bigint)
  const hasTokens = totalTokens > 0n

  // New atomic unit metrics
  const prompts = session?.userPromptCount ?? 0
  const filesEdited = session?.filesEditedCount ?? 0
  const reeditedFiles = session?.reeditedFilesCount ?? 0
  const commitCount = session?.commitCount ?? 0
  const durationSeconds = session?.durationSeconds ?? 0

  // Theme 3: Contribution metrics (optional, populated by deep index)
  const workType = session?.workType ?? null
  const aiLinesAdded = session?.aiLinesAdded ? Number(session.aiLinesAdded) : null
  const aiLinesRemoved = session?.aiLinesRemoved ? Number(session.aiLinesRemoved) : null
  const hasLoc = aiLinesAdded !== null || aiLinesRemoved !== null

  // Calculate start timestamp from modifiedAt - durationSeconds
  const endTimestamp = Number(session?.modifiedAt ?? 0)
  const startTimestamp = endTimestamp - durationSeconds

  // Branch badge data
  const gitBranch = session?.gitBranch ?? null

  // Top files data (show up to 3)
  const filesEditedPaths = session?.filesEdited ?? []
  const topFiles = filesEditedPaths.slice(0, 3)
  const remainingFiles = filesEditedPaths.length - topFiles.length

  // Extract basename from path
  const basename = (path: string): string => path.split('/').pop() || path

  return (
    <article
      className={cn(
        'w-full text-left p-3.5 rounded-lg border cursor-pointer',
        'transition-all duration-200 ease-out',
        isSelected
          ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 shadow-[0_0_0_1px_#3b82f6]'
          : 'bg-white dark:bg-gray-900 border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800 hover:border-gray-300 dark:hover:border-gray-600 hover:shadow-sm'
      )}
      aria-label={`Session: ${cleanPreview}`}
    >
      {/* Header: Project badge + Branch badge + Time range + Duration */}
      <div className="flex items-center justify-between gap-2 mb-1">
        <div className="flex items-center gap-1.5 min-w-0">
          {projectLabel && (
            <span className="inline-block px-1.5 py-0.5 text-[10px] font-medium bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded flex-shrink-0">
              {projectLabel}
            </span>
          )}
          {gitBranch && (
            <span
              className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 rounded flex-shrink-0 max-w-[160px]"
              title={gitBranch}
            >
              <GitBranch className="w-3 h-3 flex-shrink-0" />
              <span className="truncate">{gitBranch}</span>
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 text-[11px] text-gray-400 dark:text-gray-500 tabular-nums whitespace-nowrap flex-shrink-0">
          {durationSeconds > 0 ? (
            <>
              <span>{formatTimeRange(startTimestamp, endTimestamp)}</span>
              <span className="text-gray-300 dark:text-gray-600">|</span>
              <span className="font-medium text-gray-500 dark:text-gray-400">{formatDuration(durationSeconds)}</span>
            </>
          ) : (
            <span>{formatRelativeTime(Number(session.modifiedAt))}</span>
          )}
        </div>
      </div>

      {/* Preview text */}
      <div className="min-w-0">
        <p className="text-sm font-medium text-gray-900 dark:text-gray-100 line-clamp-2">
          {cleanPreview}
        </p>

        {/* Last message if different from first */}
        {cleanLast && cleanLast !== cleanPreview && (
          <p className="text-[13px] text-gray-500 dark:text-gray-400 line-clamp-1 mt-0.5">
            <span className="text-gray-300 dark:text-gray-600 mr-1">{'->'}</span>{cleanLast}
          </p>
        )}
      </div>

      {/* Metrics row: prompts, tokens, files, re-edits */}
      <div className="flex items-center gap-1 mt-2.5 text-xs text-gray-500 dark:text-gray-400">
        {prompts > 0 && (
          <>
            <span className="tabular-nums">{prompts} prompt{prompts !== 1 ? 's' : ''}</span>
            <span className="text-gray-300 dark:text-gray-600">·</span>
          </>
        )}
        {hasTokens && (
          <>
            <span className="tabular-nums">{formatNumber(totalTokens)} tokens</span>
            <span className="text-gray-300 dark:text-gray-600">·</span>
          </>
        )}
        {filesEdited > 0 && (
          <>
            <span className="tabular-nums">{filesEdited} file{filesEdited !== 1 ? 's' : ''}</span>
            <span className="text-gray-300 dark:text-gray-600">·</span>
          </>
        )}
        {reeditedFiles > 0 && (
          <span className="tabular-nums">{reeditedFiles} re-edit{reeditedFiles !== 1 ? 's' : ''}</span>
        )}
        {/* Fallback to legacy display if no new metrics */}
        {prompts === 0 && !hasTokens && filesEdited === 0 && totalTools > 0 && (
          <div className="flex items-center gap-2 text-gray-400">
            {editCount > 0 && (
              <span className="flex items-center gap-0.5" title="Edits">
                <Pencil className="w-3 h-3" />
                {editCount}
              </span>
            )}
            {toolCounts.bash > 0 && (
              <span className="flex items-center gap-0.5" title="Bash commands">
                <Terminal className="w-3 h-3" />
                {toolCounts.bash}
              </span>
            )}
            {toolCounts.read > 0 && (
              <span className="flex items-center gap-0.5" title="File reads">
                <Eye className="w-3 h-3" />
                {toolCounts.read}
              </span>
            )}
          </div>
        )}
      </div>

      {/* LOC Impact */}
      {(session.linesAdded > 0 || session.linesRemoved > 0) && (
        <div className="flex items-center gap-2 mt-2 text-xs">
          {session.locSource === 2 && (
            <GitCommit className="w-3 h-3 text-gray-400 dark:text-gray-500" />
          )}
          <span className="text-green-600 dark:text-green-400">
            +{formatNumber(session.linesAdded)}
          </span>
          <span className="text-gray-400">/</span>
          <span className="text-red-600 dark:text-red-400">
            -{formatNumber(session.linesRemoved)}
          </span>
        </div>
      )}
      {session.linesAdded === 0 && session.linesRemoved === 0 && session.locSource > 0 && (
        <div className="mt-2 text-xs text-gray-400 dark:text-gray-500">
          ±0
        </div>
      )}

      {/* Top files touched */}
      {topFiles.length > 0 && (
        <div className="flex items-center gap-1.5 mt-2.5 text-xs text-gray-500 dark:text-gray-400">
          <FileEdit className="w-3 h-3 flex-shrink-0" />
          <div className="flex items-center gap-1 min-w-0">
            {topFiles.map((file, idx) => (
              <span key={idx} className="font-mono">
                {basename(file)}
                {idx < topFiles.length - 1 && <span className="text-gray-300 dark:text-gray-600 ml-1">·</span>}
              </span>
            ))}
            {remainingFiles > 0 && (
              <span className="text-gray-400 dark:text-gray-500 ml-1">
                +{remainingFiles} more
              </span>
            )}
          </div>
        </div>
      )}

      {/* Footer: Work type badge + Commits badge + LOC + Skills */}
      <div className="flex items-center justify-between mt-2.5 pt-2.5 border-t border-gray-100 dark:border-gray-800">
        <div className="flex items-center gap-2">
          {/* Work type badge (Theme 3) */}
          {workType && (
            <WorkTypeBadge workType={workType} />
          )}

          {/* Commit badge */}
          {commitCount > 0 && (
            <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-green-50 dark:bg-green-900/30 text-green-700 dark:text-green-400 rounded border border-green-200 dark:border-green-800">
              <GitCommit className="w-3 h-3" />
              {commitCount} commit{commitCount !== 1 ? 's' : ''}
            </span>
          )}

          {/* LOC badge (Theme 3) */}
          {hasLoc && (
            <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-gray-50 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded border border-gray-200 dark:border-gray-700 tabular-nums">
              <Code2 className="w-3 h-3" />
              {aiLinesAdded !== null && aiLinesAdded > 0 && (
                <span className="text-green-600 dark:text-green-400">+{formatNumber(aiLinesAdded)}</span>
              )}
              {aiLinesRemoved !== null && aiLinesRemoved > 0 && (
                <span className="text-red-600 dark:text-red-400">-{formatNumber(aiLinesRemoved)}</span>
              )}
            </span>
          )}

          {/* Skills used */}
          {(session.skillsUsed?.length ?? 0) > 0 && (
            <div className="flex items-center gap-1">
              {session.skillsUsed?.slice(0, 2).map(skill => (
                <span
                  key={skill}
                  className="px-1.5 py-0.5 text-xs bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded font-mono"
                >
                  {skill}
                </span>
              ))}
              {(session.skillsUsed?.length ?? 0) > 2 && (
                <span className="text-xs text-gray-400">
                  +{(session.skillsUsed?.length ?? 0) - 2}
                </span>
              )}
            </div>
          )}
        </div>

        {/* Message/turn count (if available and useful) */}
        {(session.messageCount ?? 0) > 0 && prompts === 0 && (
          <div className="flex items-center gap-1 text-xs text-gray-400">
            <MessageSquare className="w-3 h-3" />
            <span>
              {session.messageCount} msgs
            </span>
          </div>
        )}
      </div>
    </article>
  )
}
