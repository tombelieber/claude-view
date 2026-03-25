import * as ContextMenu from '@radix-ui/react-context-menu'
import * as Tooltip from '@radix-ui/react-tooltip'
import {
  Archive,
  Code2,
  DollarSign,
  Eye,
  FileEdit,
  FolderOpen,
  GitBranch,
  GitCommit,
  MessageSquare,
  Pencil,
  Terminal,
  UsersRound,
} from 'lucide-react'
import type { SessionInfo } from '../hooks/use-projects'
import { useTeamDetail, useTeamForSession } from '../hooks/use-teams'
import { formatCostUsd, formatNumber } from '../lib/format-utils'
import { computeWeight, weightBorderClass } from '../lib/session-weight'
import { getDisplayLongestTaskSeconds, getDisplayTaskTimeSeconds } from '../lib/task-time-utils'
import { cn } from '../lib/utils'
import { cleanPreviewText, getSessionTitle } from '../utils/get-session-title'
import { CategoryBadge } from './CategoryBadge'
import { WeightIndicator } from './WeightIndicator'
import { WorkTypeBadge } from './WorkTypeBadge'
import { TeamMemberPills } from './live/TeamMemberPills'
import { SessionSpinner, formatDurationCompact, pickPastVerb } from './spinner'

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
  isLive?: boolean
  projectDisplayName?: string | null
  onResumeClick?: (sessionId: string) => void
  onArchive?: (sessionId: string) => void
  selectable?: boolean
  selected?: boolean
  onSelectToggle?: (sessionId: string) => void
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
    return (
      date.toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
      }) + `, ${timeStr}`
    )
  }
}

export function SessionCard({
  session,
  isSelected = false,
  isLive = false,
  projectDisplayName,
  onResumeClick,
  onArchive,
  selectable = false,
  selected = false,
  onSelectToggle,
}: SessionCardProps) {
  // Hooks must be called BEFORE early return to satisfy Rules of Hooks
  const teamMatch = useTeamForSession(session?.id)
  const { data: teamDetail } = useTeamDetail(teamMatch?.name ?? null)

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
  const totalTokens = (session?.totalInputTokens ?? 0) + (session?.totalOutputTokens ?? 0)
  const hasTokens = totalTokens > 0

  // New atomic unit metrics
  const prompts = session?.userPromptCount ?? 0
  const filesEdited = session?.filesEditedCount ?? 0
  const reeditedFiles = session?.reeditedFilesCount ?? 0
  const commitCount = session?.commitCount ?? 0
  const durationSeconds = session?.durationSeconds ?? 0

  // Task time metrics
  const displayTaskTimeSeconds = getDisplayTaskTimeSeconds(session) ?? durationSeconds
  const longestTaskSeconds = getDisplayLongestTaskSeconds(session) ?? 0
  const taskTimeSeconds = getDisplayTaskTimeSeconds(session)

  // Cost
  const totalCostUsd = session?.totalCostUsd ?? null
  const hasCost = totalCostUsd !== null && totalCostUsd > 0

  // Theme 3: Contribution metrics (optional, populated by deep index)
  const workType = session?.workType ?? null
  const aiLinesAdded = session?.aiLinesAdded ? Number(session.aiLinesAdded) : null
  const aiLinesRemoved = session?.aiLinesRemoved ? Number(session.aiLinesRemoved) : null
  const hasLoc = aiLinesAdded !== null || aiLinesRemoved !== null

  // Session weight for visual accent
  const weightTier = computeWeight({
    totalTokens,
    userPromptCount: prompts,
    filesEditedCount: filesEdited,
    durationSeconds,
  })

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
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>
        <article
          className={cn(
            'group relative w-full text-left p-3.5 rounded-lg border border-l-[3px] cursor-pointer',
            'transition-all duration-200 ease-out',
            isSelected
              ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 shadow-[0_0_0_1px_#3b82f6]'
              : isLive
                ? 'bg-green-50/40 dark:bg-green-950/20 border-green-300 dark:border-green-800 border-l-green-500 dark:border-l-green-500 hover:bg-green-50/70 dark:hover:bg-green-950/30 hover:shadow-sm'
                : cn(
                    'bg-white dark:bg-gray-900 border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800 hover:border-gray-300 dark:hover:border-gray-600 hover:shadow-sm',
                    weightBorderClass(weightTier),
                  ),
          )}
          aria-label={`Session: ${cleanPreview}`}
        >
          {selectable && (
            <input
              type="checkbox"
              checked={selected}
              onChange={() => onSelectToggle?.(session.id)}
              className="absolute top-2 left-2 z-10"
              onClick={(e) => e.stopPropagation()}
            />
          )}
          {onArchive && !selectable && (
            <button
              type="button"
              onClick={(e) => {
                e.preventDefault()
                e.stopPropagation()
                onArchive(session.id)
              }}
              className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity p-1.5 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700"
              title="Archive session"
            >
              <Archive className="w-4 h-4 text-gray-500" />
            </button>
          )}
          {/* Header: Weight dot + Project badge + Branch badge + Time range + Duration */}
          <div className="flex items-center justify-between gap-2 mb-1">
            <div className="flex items-center gap-1.5 min-w-0">
              <WeightIndicator tier={weightTier} />
              {projectLabel && (
                <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-medium text-gray-700 dark:text-gray-300 rounded shrink-0">
                  <FolderOpen className="w-3 h-3 text-amber-500 dark:text-amber-400 shrink-0" />
                  {projectLabel}
                </span>
              )}
              {gitBranch && (
                <span
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-mono bg-violet-50 dark:bg-violet-950/50 border border-violet-200 dark:border-violet-800 text-violet-700 dark:text-violet-300 rounded shrink-0 max-w-40"
                  title={gitBranch}
                >
                  <GitBranch className="w-3 h-3 shrink-0" />
                  <span className="truncate">{gitBranch}</span>
                </span>
              )}
              {isLive && (
                <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-semibold bg-green-50 dark:bg-green-950/40 text-green-700 dark:text-green-400 rounded flex-shrink-0 border border-green-200 dark:border-green-800/60">
                  <span className="relative flex h-2 w-2">
                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75" />
                    <span className="relative inline-flex rounded-full h-2 w-2 bg-green-500" />
                  </span>
                  LIVE
                </span>
              )}
              {teamMatch && (
                <Tooltip.Provider delayDuration={200}>
                  <Tooltip.Root>
                    <Tooltip.Trigger asChild>
                      <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-medium rounded bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 cursor-default">
                        <UsersRound className="w-3 h-3" />
                        Teams &middot; {teamMatch.memberCount} agents
                      </span>
                    </Tooltip.Trigger>
                    <Tooltip.Portal>
                      <Tooltip.Content
                        className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs"
                        sideOffset={5}
                      >
                        <p className="font-medium text-gray-900 dark:text-gray-100">
                          Agent Team: {teamMatch.name}
                        </p>
                        <p className="text-gray-500 dark:text-gray-400 mt-0.5">
                          {teamMatch.memberCount} team members. Click to view team details in the
                          Teams tab.
                        </p>
                        <Tooltip.Arrow className="fill-gray-200 dark:fill-gray-700" />
                      </Tooltip.Content>
                    </Tooltip.Portal>
                  </Tooltip.Root>
                </Tooltip.Provider>
              )}
            </div>
            <div className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500 tabular-nums whitespace-nowrap flex-shrink-0">
              {durationSeconds > 0 ? (
                <>
                  <span>{formatTimeRange(startTimestamp, endTimestamp)}</span>
                  <span className="text-gray-300 dark:text-gray-600">|</span>
                  <span className="font-medium text-gray-500 dark:text-gray-400">
                    {formatDuration(displayTaskTimeSeconds)}
                  </span>
                </>
              ) : (
                <span>{formatRelativeTime(Number(session.modifiedAt))}</span>
              )}
              {hasCost && (
                <>
                  <span className="text-gray-300 dark:text-gray-600">|</span>
                  <span className="inline-flex items-center gap-0.5 font-medium text-amber-600 dark:text-amber-400">
                    <DollarSign className="w-3 h-3" />
                    {formatCostUsd(totalCostUsd!).replace('$', '')}
                  </span>
                </>
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
              <p className="text-xs text-gray-500 dark:text-gray-400 line-clamp-1 mt-0.5">
                <span className="text-gray-300 dark:text-gray-600 mr-1">{'->'}</span>
                {cleanLast}
              </p>
            )}
          </div>

          {/* Spinner: verb + task time + model */}
          {session.primaryModel && (
            <div className="mt-1.5">
              <SessionSpinner
                mode="historical"
                model={session.primaryModel}
                pastTenseVerb={pickPastVerb(session.id)}
                taskTimeSeconds={taskTimeSeconds}
              />
            </div>
          )}

          {/* Team member pills — with label */}
          {teamDetail && teamDetail.members.length > 0 && (
            <div className="mt-2 -mx-1 px-1">
              <span className="text-xs font-medium uppercase tracking-wider text-zinc-400 dark:text-zinc-500 mb-0.5 block">
                Team Members
              </span>
              <TeamMemberPills members={teamDetail.members} />
            </div>
          )}

          {/* Metrics box */}
          {(prompts > 0 || hasTokens || filesEdited > 0 || totalTools > 0) && (
            <div className="mt-2.5 rounded-md bg-gray-50 dark:bg-gray-800/50 px-3 py-2 text-xs text-gray-500 dark:text-gray-400">
              {/* Row 1: Usage — prompts, tokens, longest task */}
              {(prompts > 0 || hasTokens || longestTaskSeconds > 0) && (
                <div className="flex items-center gap-3">
                  {prompts > 0 && (
                    <span className="tabular-nums">
                      {prompts} prompt{prompts !== 1 ? 's' : ''}
                    </span>
                  )}
                  {hasTokens && (
                    <span className="tabular-nums">{formatNumber(totalTokens)} tokens</span>
                  )}
                  {longestTaskSeconds > 0 && (
                    <span className="tabular-nums">
                      longest {formatDurationCompact(longestTaskSeconds)}
                    </span>
                  )}
                </div>
              )}
              {/* Row 2: Output — files, re-edits, LOC */}
              {(filesEdited > 0 ||
                reeditedFiles > 0 ||
                session.linesAdded > 0 ||
                session.linesRemoved > 0) && (
                <div className="flex items-center gap-3 mt-1">
                  {filesEdited > 0 && (
                    <span className="tabular-nums">
                      {filesEdited} file{filesEdited !== 1 ? 's' : ''}
                    </span>
                  )}
                  {reeditedFiles > 0 && (
                    <span className="tabular-nums">
                      {reeditedFiles} re-edit{reeditedFiles !== 1 ? 's' : ''}
                    </span>
                  )}
                  {(session.linesAdded > 0 || session.linesRemoved > 0) && (
                    <span className="inline-flex items-center gap-1.5 tabular-nums">
                      {session.locSource === 2 && (
                        <GitCommit className="w-3 h-3 text-gray-400 dark:text-gray-500" />
                      )}
                      <span className="text-green-600 dark:text-green-400">
                        +{formatNumber(session.linesAdded)}
                      </span>
                      <span className="text-gray-300 dark:text-gray-600">/</span>
                      <span className="text-red-600 dark:text-red-400">
                        -{formatNumber(session.linesRemoved)}
                      </span>
                    </span>
                  )}
                </div>
              )}
              {/* Legacy fallback for old sessions without new metrics */}
              {prompts === 0 && !hasTokens && filesEdited === 0 && totalTools > 0 && (
                <div className="flex items-center gap-3">
                  {editCount > 0 && (
                    <span className="flex items-center gap-1" title="Edits">
                      <Pencil className="w-3 h-3" />
                      <span className="tabular-nums">{editCount}</span>
                    </span>
                  )}
                  {toolCounts.bash > 0 && (
                    <span className="flex items-center gap-1" title="Bash commands">
                      <Terminal className="w-3 h-3" />
                      <span className="tabular-nums">{toolCounts.bash}</span>
                    </span>
                  )}
                  {toolCounts.read > 0 && (
                    <span className="flex items-center gap-1" title="File reads">
                      <Eye className="w-3 h-3" />
                      <span className="tabular-nums">{toolCounts.read}</span>
                    </span>
                  )}
                </div>
              )}
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
                    {idx < topFiles.length - 1 && (
                      <span className="text-gray-300 dark:text-gray-600 ml-1">·</span>
                    )}
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
              {/* AI classification badge → rule-based WorkType */}
              {session.categoryL2 ? (
                <CategoryBadge
                  l1={session.categoryL1}
                  l2={session.categoryL2}
                  l3={session.categoryL3}
                />
              ) : workType ? (
                <WorkTypeBadge workType={workType} />
              ) : null}

              {/* Commit badge */}
              {commitCount > 0 && (
                <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-gray-50 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded border border-gray-200 dark:border-gray-700">
                  <GitCommit className="w-3 h-3" />
                  {commitCount} commit{commitCount !== 1 ? 's' : ''}
                </span>
              )}

              {/* LOC badge (Theme 3) */}
              {hasLoc && (
                <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-gray-50 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded border border-gray-200 dark:border-gray-700 tabular-nums">
                  <Code2 className="w-3 h-3" />
                  {aiLinesAdded !== null && aiLinesAdded > 0 && (
                    <span className="text-green-600 dark:text-green-400">
                      +{formatNumber(aiLinesAdded)}
                    </span>
                  )}
                  {aiLinesRemoved !== null && aiLinesRemoved > 0 && (
                    <span className="text-red-600 dark:text-red-400">
                      -{formatNumber(aiLinesRemoved)}
                    </span>
                  )}
                </span>
              )}

              {/* Skills used */}
              {(session.skillsUsed?.length ?? 0) > 0 && (
                <div className="flex items-center gap-1">
                  {session.skillsUsed?.slice(0, 2).map((skill) => (
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

            <div className="flex items-center gap-2">
              {/* Resume button (only when handler provided) */}
              {onResumeClick && session.id && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation()
                    e.preventDefault()
                    onResumeClick(session.id)
                  }}
                  className="px-2 py-0.5 text-xs font-medium text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20 rounded hover:bg-blue-100 dark:hover:bg-blue-900/40 transition-colors"
                >
                  Resume
                </button>
              )}

              {/* Turn/message count — always visible for scanning session length */}
              {((session.turnCount ?? 0) > 0 || (session.messageCount ?? 0) > 0) && (
                <div className="flex items-center gap-1 text-xs text-gray-400 tabular-nums">
                  <MessageSquare className="w-3 h-3" />
                  <span>
                    {(session.turnCount ?? 0) > 0
                      ? `${session.turnCount} turn${session.turnCount !== 1 ? 's' : ''}`
                      : `${session.messageCount} msg${(session.messageCount ?? 0) !== 1 ? 's' : ''}`}
                  </span>
                </div>
              )}
            </div>
          </div>
        </article>
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="min-w-40 bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700 p-1 z-50">
          <ContextMenu.Item
            className="flex items-center gap-2 px-3 py-2 text-sm rounded-md cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300"
            onSelect={() => onArchive?.(session.id)}
          >
            <Archive className="w-4 h-4" />
            Archive session
          </ContextMenu.Item>
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}
