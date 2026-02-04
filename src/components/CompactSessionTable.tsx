import { Link } from 'react-router-dom'
import { ArrowUp, ArrowDown, GitBranch } from 'lucide-react'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'
import { sessionSlug } from '../lib/url-slugs'
import type { SessionInfo } from '../hooks/use-projects'

export type SortColumn = 'time' | 'branch' | 'prompts' | 'tokens' | 'files' | 'loc' | 'commits' | 'duration'
export type SortDirection = 'asc' | 'desc'

interface CompactSessionTableProps {
  sessions: SessionInfo[]
  onSort: (column: SortColumn) => void
  sortColumn: SortColumn
  sortDirection: SortDirection
}

/**
 * Format timestamp as date prefix (Today/Yesterday/Jan 26)
 */
function formatDatePrefix(timestamp: number): string {
  const date = new Date(timestamp * 1000)
  const now = new Date()

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
 * Format time as HH:MM AM/PM
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
 * Format time range: "Today 2:30 PM -> 3:15 PM"
 */
function formatTimeRange(startTimestamp: number, endTimestamp: number): string {
  const prefix = formatDatePrefix(startTimestamp)
  const startTime = formatTimeOnly(startTimestamp)
  const endTime = formatTimeOnly(endTimestamp)
  return `${prefix} ${startTime} -> ${endTime}`
}

/**
 * Format duration: "45m" or "2.1h"
 */
function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = seconds / 3600
  return `${hours.toFixed(1)}h`
}

/**
 * Format token count with K/M suffix
 */
function formatTokens(inputTokens: bigint | null, outputTokens: bigint | null): string {
  const total = Number((inputTokens ?? 0n) + (outputTokens ?? 0n))
  if (total === 0) return '--'
  if (total >= 1_000_000) return `${(total / 1_000_000).toFixed(1)}M`
  if (total >= 1_000) return `${Math.round(total / 1_000)}K`
  return total.toLocaleString()
}

/**
 * Format LOC as "+N / -N"
 */
function formatLOC(added: number, removed: number): string {
  if (added === 0 && removed === 0) return '--'
  return `+${formatNumber(added)} / -${formatNumber(removed)}`
}

interface ColumnHeaderProps {
  label: string
  column: SortColumn | null
  sortColumn: SortColumn
  sortDirection: SortDirection
  onSort?: (column: SortColumn) => void
  align?: 'left' | 'right'
  width?: string
}

function ColumnHeader({ label, column, sortColumn, sortDirection, onSort, align = 'left', width }: ColumnHeaderProps) {
  const isSortable = column !== null
  const isSorted = isSortable && column === sortColumn

  const headerContent = (
    <>
      <span>{label}</span>
      {isSorted && (
        <span className="ml-1">
          {sortDirection === 'asc' ? (
            <ArrowUp className="w-3 h-3 inline" />
          ) : (
            <ArrowDown className="w-3 h-3 inline" />
          )}
        </span>
      )}
    </>
  )

  const thClasses = cn(
    'py-2 px-3 text-xs font-semibold text-gray-600 dark:text-gray-400 border-b border-gray-200 dark:border-gray-700',
    align === 'right' ? 'text-right' : 'text-left'
  )

  return (
    <th
      scope="col"
      className={thClasses}
      style={width ? { width } : undefined}
      aria-sort={isSortable ? (isSorted ? (sortDirection === 'asc' ? 'ascending' : 'descending') : 'other') : 'none'}
    >
      {isSortable && onSort ? (
        <button
          type="button"
          onClick={() => onSort(column!)}
          className={cn(
            'inline-flex items-center gap-1 transition-colors',
            'hover:text-gray-900 dark:hover:text-gray-200',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded',
            isSorted && 'text-blue-600 dark:text-blue-400 font-bold'
          )}
          aria-label={`Sort by ${label}`}
        >
          {headerContent}
        </button>
      ) : (
        headerContent
      )}
    </th>
  )
}

interface TableRowProps {
  session: SessionInfo
}

function TableRow({ session }: TableRowProps) {
  const endTimestamp = Number(session.modifiedAt)
  const startTimestamp = endTimestamp - session.durationSeconds

  const totalTokens = formatTokens(session.totalInputTokens, session.totalOutputTokens)
  const loc = formatLOC(session.linesAdded, session.linesRemoved)
  const duration = session.durationSeconds > 0 ? formatDuration(session.durationSeconds) : '--'

  const sessionUrl = `/project/${encodeURIComponent(session.project)}/session/${sessionSlug(session.preview, session.id)}`

  return (
    <tr
      className="border-b border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors cursor-pointer"
    >
      <td className="py-2 px-3">
        <Link
          to={sessionUrl}
          className="block text-xs text-gray-900 dark:text-gray-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded"
        >
          {formatTimeRange(startTimestamp, endTimestamp)}
        </Link>
      </td>
      <td className="py-2 px-3">
        <Link to={sessionUrl} className="block">
          {session.gitBranch ? (
            <span className="inline-flex items-center gap-1 text-xs font-mono text-gray-600 dark:text-gray-400 max-w-[120px]">
              <GitBranch className="w-3 h-3 flex-shrink-0" />
              <span className="truncate">{session.gitBranch}</span>
            </span>
          ) : (
            <span className="text-xs text-gray-400">--</span>
          )}
        </Link>
      </td>
      <td className="py-2 px-3">
        <Link to={sessionUrl} className="block max-w-full">
          <span className="text-sm text-gray-900 dark:text-gray-100 truncate block">
            {session.preview || 'Untitled session'}
          </span>
        </Link>
      </td>
      <td className="py-2 px-3 text-right tabular-nums">
        <Link to={sessionUrl} className="block text-sm text-gray-900 dark:text-gray-100">
          {session.userPromptCount}
        </Link>
      </td>
      <td className="py-2 px-3 text-right tabular-nums">
        <Link to={sessionUrl} className="block text-sm text-gray-900 dark:text-gray-100">
          {totalTokens}
        </Link>
      </td>
      <td className="py-2 px-3 text-right tabular-nums">
        <Link to={sessionUrl} className="block text-sm text-gray-900 dark:text-gray-100">
          {session.filesEditedCount}
        </Link>
      </td>
      <td className="py-2 px-3 text-right tabular-nums text-sm">
        <Link to={sessionUrl} className="block">
          {loc !== '--' ? (
            <>
              <span className="text-green-600 dark:text-green-400">+{formatNumber(session.linesAdded)}</span>
              <span className="text-gray-400 mx-0.5">/</span>
              <span className="text-red-600 dark:text-red-400">-{formatNumber(session.linesRemoved)}</span>
            </>
          ) : (
            <span className="text-gray-400">--</span>
          )}
        </Link>
      </td>
      <td className="py-2 px-3 text-right tabular-nums">
        <Link to={sessionUrl} className="block text-sm text-gray-900 dark:text-gray-100">
          {session.commitCount > 0 ? session.commitCount : <span className="text-gray-400">--</span>}
        </Link>
      </td>
      <td className="py-2 px-3 text-right tabular-nums">
        <Link to={sessionUrl} className="block text-sm text-gray-900 dark:text-gray-100">
          {duration}
        </Link>
      </td>
    </tr>
  )
}

/**
 * CompactSessionTable component for table view mode.
 *
 * Features:
 * - 9 columns: Time, Branch, Preview, Prompts, Tokens, Files, LOC, Commits, Duration
 * - Click row to navigate to session detail
 * - Click column header to sort
 * - Horizontal scroll on mobile
 * - Hover highlights
 * - Tabular nums on numeric columns
 *
 * @example
 * ```tsx
 * <CompactSessionTable
 *   sessions={sessions}
 *   onSort={(col) => handleSort(col)}
 *   sortColumn="tokens"
 *   sortDirection="desc"
 * />
 * ```
 */
export function CompactSessionTable({ sessions, onSort, sortColumn, sortDirection }: CompactSessionTableProps) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse bg-white dark:bg-gray-900 rounded-lg overflow-hidden" role="table">
        <thead>
          <tr>
            <ColumnHeader
              label="Time"
              column="time"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              width="140px"
            />
            <ColumnHeader
              label="Branch"
              column="branch"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              width="120px"
            />
            <ColumnHeader
              label="Preview"
              column={null}
              sortColumn={sortColumn}
              sortDirection={sortDirection}
            />
            <ColumnHeader
              label="Prompts"
              column="prompts"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              align="right"
              width="60px"
            />
            <ColumnHeader
              label="Tokens"
              column="tokens"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              align="right"
              width="70px"
            />
            <ColumnHeader
              label="Files"
              column="files"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              align="right"
              width="50px"
            />
            <ColumnHeader
              label="LOC"
              column="loc"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              align="right"
              width="80px"
            />
            <ColumnHeader
              label="Commits"
              column="commits"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              align="right"
              width="60px"
            />
            <ColumnHeader
              label="Duration"
              column="duration"
              sortColumn={sortColumn}
              sortDirection={sortDirection}
              onSort={onSort}
              align="right"
              width="70px"
            />
          </tr>
        </thead>
        <tbody>
          {sessions.map((session) => (
            <TableRow key={session.id} session={session} />
          ))}
        </tbody>
      </table>
    </div>
  )
}
