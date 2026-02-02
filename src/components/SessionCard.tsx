import { Terminal, Pencil, Eye, MessageSquare, GitCommit } from 'lucide-react'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'
import type { SessionInfo } from '../hooks/use-projects'

interface SessionCardProps {
  session: SessionInfo | null | undefined
  isSelected?: boolean
  projectDisplayName?: string | null
}

/** Strip XML tags, system prompt noise, and quotes from preview text */
function cleanPreviewText(text: string): string {
  // Remove XML-like tags
  let cleaned = text.replace(/<[^>]+>/g, '')
  // Remove leading/trailing quotes
  cleaned = cleaned.replace(/^["']|["']$/g, '')
  // Remove slash-command prefixes like "/superpowers:brainstorm"
  cleaned = cleaned.replace(/\/[\w-]+:[\w-]+\s*/g, '')
  // Remove "superpowers:" prefixed words
  cleaned = cleaned.replace(/superpowers:\S+\s*/g, '')
  // Collapse whitespace
  cleaned = cleaned.replace(/\s+/g, ' ').trim()
  // If it starts with common system prompt patterns, show a clean label
  if (cleaned.startsWith('You are a ') || cleaned.startsWith('You are Claude')) {
    return 'System prompt session'
  }
  // If it looks like ls output or file listing
  if (cleaned.match(/^"?\s*total \d+/)) {
    return cleaned.slice(0, 100) + (cleaned.length > 100 ? '...' : '')
  }
  return cleaned || 'Untitled session'
}

/**
 * Format time for session card display.
 * Returns format like "2:30 PM" for use in time ranges.
 */
function formatTimeOnly(timestamp: number): string {
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

  // Clean up preview text: strip XML tags and system prompt noise
  const cleanPreview = cleanPreviewText(session?.preview || '')
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

  // Calculate start timestamp from modifiedAt - durationSeconds
  const endTimestamp = Number(session?.modifiedAt ?? 0)
  const startTimestamp = endTimestamp - durationSeconds

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
      {/* Header: Project badge + Time range + Duration */}
      <div className="flex items-center justify-between gap-2 mb-1">
        <div className="flex items-center gap-1.5 min-w-0">
          {projectLabel && (
            <span className="inline-block px-1.5 py-0.5 text-[10px] font-medium bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded flex-shrink-0">
              {projectLabel}
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

      {/* Footer: Commits badge + Skills */}
      <div className="flex items-center justify-between mt-2.5 pt-2.5 border-t border-gray-100 dark:border-gray-800">
        <div className="flex items-center gap-2">
          {/* Commit badge */}
          {commitCount > 0 && (
            <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-green-50 dark:bg-green-900/30 text-green-700 dark:text-green-400 rounded border border-green-200 dark:border-green-800">
              <GitCommit className="w-3 h-3" />
              {commitCount} commit{commitCount !== 1 ? 's' : ''}
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
