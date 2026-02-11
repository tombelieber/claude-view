import { useMemo } from 'react'
import { Link } from 'react-router-dom'
import {
  createColumnHelper,
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
  type SortingState,
} from '@tanstack/react-table'
import { ArrowUp, ArrowDown, GitBranch, GitCommit, Search } from 'lucide-react'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'
import { buildSessionUrl } from '../lib/url-utils'
import type { SessionInfo } from '../hooks/use-projects'
import { getSessionTitle } from '../utils/get-session-title'

export type SortColumn = 'time' | 'branch' | 'prompts' | 'files' | 'commits' | 'duration'
export type SortDirection = 'asc' | 'desc'

interface CompactSessionTableProps {
  sessions: SessionInfo[]
  onSort: (column: SortColumn) => void
  sortColumn: SortColumn
  sortDirection: SortDirection
}

// --- Formatters ---

function formatDatePrefix(timestamp: number): string {
  if (timestamp <= 0) return '--'
  const date = new Date(timestamp * 1000)
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const targetDay = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  const diffDays = Math.floor((today.getTime() - targetDay.getTime()) / (1000 * 60 * 60 * 24))
  if (diffDays === 0) return 'Today'
  if (diffDays === 1) return 'Yest.'
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

function formatTimeShort(timestamp: number): string {
  if (timestamp <= 0) return '--'
  return new Date(timestamp * 1000).toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  return `${(seconds / 3600).toFixed(1)}h`
}

function formatTokens(inputTokens: bigint | null, outputTokens: bigint | null): string {
  const total = Number((inputTokens ?? 0n) + (outputTokens ?? 0n))
  if (total === 0) return '0'
  if (total >= 1_000_000) return `${(total / 1_000_000).toFixed(1)}M`
  if (total >= 1_000) return `${Math.round(total / 1_000)}K`
  return total.toLocaleString()
}

function sessionUrl(session: SessionInfo): string {
  return buildSessionUrl(session.id)
}

// --- Column definitions ---

const columnHelper = createColumnHelper<SessionInfo>()

const columns: ColumnDef<SessionInfo, any>[] = [
  columnHelper.accessor('modifiedAt', {
    id: 'time',
    header: 'Time',
    size: 115,
    enableSorting: true,
    cell: ({ row }) => {
      const s = row.original
      const end = Number(s.modifiedAt)
      return (
        <Link to={sessionUrl(s)} className="block whitespace-nowrap text-[12px] text-gray-700 dark:text-gray-300">
          <span className="font-medium text-gray-900 dark:text-gray-100">{formatDatePrefix(end)}</span>
          {' '}
          <span className="text-gray-400 dark:text-gray-500">{formatTimeShort(end)}</span>
        </Link>
      )
    },
  }),
  columnHelper.accessor('gitBranch', {
    id: 'branch',
    header: 'Branch',
    size: 120,
    enableSorting: true,
    cell: ({ row }) => {
      const s = row.original
      return (
        <Link to={sessionUrl(s)} className="block">
          {s.gitBranch ? (
            <span className="inline-flex items-center gap-1 max-w-full px-1.5 py-0.5 rounded-md bg-gray-100 dark:bg-gray-800 border border-gray-200/60 dark:border-gray-700/60 text-[11px] font-mono text-gray-600 dark:text-gray-400">
              <GitBranch className="w-3 h-3 flex-shrink-0 text-gray-400 dark:text-gray-500" />
              <span className="truncate">{s.gitBranch}</span>
            </span>
          ) : (
            <span className="text-[11px] text-gray-300 dark:text-gray-600">--</span>
          )}
        </Link>
      )
    },
  }),
  columnHelper.accessor('preview', {
    id: 'preview',
    header: 'Preview',
    enableSorting: false,
    cell: ({ row }) => {
      const s = row.original
      return (
        <Link to={sessionUrl(s)} className="block">
          <span className="text-[13px] font-medium text-gray-900 dark:text-gray-100 truncate block">
            {getSessionTitle(s.preview, s.summary)}
          </span>
        </Link>
      )
    },
  }),
  columnHelper.accessor('userPromptCount', {
    id: 'prompts',
    header: 'Activity',
    size: 85,
    enableSorting: true,
    meta: { align: 'right' },
    cell: ({ row }) => {
      const s = row.original
      const totalTokens = formatTokens(s.totalInputTokens, s.totalOutputTokens)
      const hasActivity = s.userPromptCount > 0 || totalTokens !== '0'
      return (
        <Link to={sessionUrl(s)} className="block whitespace-nowrap">
          {hasActivity ? (
            <span className="text-[12px]">
              <span className="font-medium text-gray-900 dark:text-gray-100">{s.userPromptCount}</span>
              <span className="text-gray-300 dark:text-gray-600 mx-0.5">/</span>
              <span className="text-gray-500 dark:text-gray-400">{totalTokens}</span>
            </span>
          ) : (
            <span className="text-[12px] text-gray-300 dark:text-gray-600">--</span>
          )}
        </Link>
      )
    },
  }),
  columnHelper.accessor('filesEditedCount', {
    id: 'files',
    header: 'Changes',
    size: 100,
    enableSorting: true,
    meta: { align: 'right' },
    cell: ({ row }) => {
      const s = row.original
      const hasLOC = s.linesAdded > 0 || s.linesRemoved > 0
      return (
        <Link to={sessionUrl(s)} className="block whitespace-nowrap">
          {s.filesEditedCount > 0 ? (
            <span className="text-[12px]">
              <span className="font-medium text-gray-900 dark:text-gray-100">{s.filesEditedCount}f</span>
              {hasLOC && (
                <>
                  {' '}
                  <span className="text-green-600 dark:text-green-400">+{formatNumber(s.linesAdded)}</span>
                  <span className="text-gray-300 dark:text-gray-600">/</span>
                  <span className="text-red-500 dark:text-red-400">-{formatNumber(s.linesRemoved)}</span>
                </>
              )}
            </span>
          ) : (
            <span className="text-[12px] text-gray-300 dark:text-gray-600">--</span>
          )}
        </Link>
      )
    },
  }),
  columnHelper.accessor('commitCount', {
    id: 'commits',
    header: 'Commits',
    size: 62,
    enableSorting: true,
    meta: { align: 'right' },
    cell: ({ row }) => {
      const s = row.original
      return (
        <Link to={sessionUrl(s)} className="block">
          {s.commitCount > 0 ? (
            <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-md bg-emerald-50 dark:bg-emerald-950/40 text-emerald-700 dark:text-emerald-400 text-[11px] font-semibold tabular-nums">
              <GitCommit className="w-3 h-3" />
              {s.commitCount}
            </span>
          ) : (
            <span className="text-[12px] text-gray-300 dark:text-gray-600">--</span>
          )}
        </Link>
      )
    },
  }),
  columnHelper.accessor('durationSeconds', {
    id: 'duration',
    header: 'Dur.',
    size: 52,
    enableSorting: true,
    meta: { align: 'right' },
    cell: ({ row }) => {
      const s = row.original
      const duration = s.durationSeconds > 0 ? formatDuration(s.durationSeconds) : null
      return (
        <Link to={sessionUrl(s)} className="block text-[12px]">
          {duration ? (
            <span className="font-medium text-gray-700 dark:text-gray-300">{duration}</span>
          ) : (
            <span className="text-gray-300 dark:text-gray-600">--</span>
          )}
        </Link>
      )
    },
  }),
]

// --- Component ---

export function CompactSessionTable({ sessions, onSort, sortColumn, sortDirection }: CompactSessionTableProps) {
  const sorting: SortingState = useMemo(
    () => [{ id: sortColumn, desc: sortDirection === 'desc' }],
    [sortColumn, sortDirection]
  )

  const table = useReactTable({
    data: sessions,
    columns,
    state: { sorting },
    onSortingChange: (updater) => {
      const next = typeof updater === 'function' ? updater(sorting) : updater
      if (next.length > 0) {
        onSort(next[0].id as SortColumn)
      }
    },
    getCoreRowModel: getCoreRowModel(),
    manualSorting: true,
    enableSortingRemoval: false,
  })

  if (sessions.length === 0) {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 shadow-sm bg-white dark:bg-gray-900">
        <div className="flex flex-col items-center justify-center py-16 px-4">
          <div className="w-12 h-12 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center mb-4">
            <Search className="w-5 h-5 text-gray-400" />
          </div>
          <h3 className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-1">No sessions found</h3>
          <p className="text-xs text-gray-500 dark:text-gray-400">Try adjusting your filters</p>
        </div>
      </div>
    )
  }

  return (
    <div className="overflow-x-auto rounded-lg border border-gray-200 dark:border-gray-700 shadow-sm">
      <table className="w-full table-fixed border-collapse bg-white dark:bg-gray-900" role="table">
        <thead className="sticky top-0 z-10 bg-gray-50 dark:bg-gray-800/90 backdrop-blur-sm">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => {
                const align = (header.column.columnDef.meta as { align?: string })?.align ?? 'left'
                const isSorted = header.column.getIsSorted()
                const canSort = header.column.getCanSort()
                // For the flex-grow preview column, don't set explicit width
                const isPreview = header.id === 'preview'
                return (
                  <th
                    key={header.id}
                    scope="col"
                    className={cn(
                      'py-2 px-3 uppercase tracking-wider text-[10px] font-semibold text-gray-400 dark:text-gray-500 border-b border-gray-200 dark:border-gray-700',
                      align === 'right' ? 'text-right' : 'text-left'
                    )}
                    style={isPreview ? undefined : { width: header.getSize() }}
                    aria-sort={canSort ? (isSorted ? (isSorted === 'asc' ? 'ascending' : 'descending') : 'other') : 'none'}
                  >
                    {canSort ? (
                      <button
                        type="button"
                        onClick={header.column.getToggleSortingHandler()}
                        className={cn(
                          'inline-flex items-center gap-0.5 transition-colors rounded-sm px-1 -mx-1',
                          'hover:text-gray-600 dark:hover:text-gray-300',
                          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                          isSorted && 'text-blue-600 dark:text-blue-400 font-bold'
                        )}
                        aria-label={`Sort by ${flexRender(header.column.columnDef.header, header.getContext())}`}
                      >
                        <span>{flexRender(header.column.columnDef.header, header.getContext())}</span>
                        {isSorted && (
                          <span className="ml-0.5">
                            {isSorted === 'asc' ? (
                              <ArrowUp className="w-3 h-3 inline" />
                            ) : (
                              <ArrowDown className="w-3 h-3 inline" />
                            )}
                          </span>
                        )}
                      </button>
                    ) : (
                      <span>{flexRender(header.column.columnDef.header, header.getContext())}</span>
                    )}
                  </th>
                )
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row, idx) => (
            <tr
              key={row.id}
              className={cn(
                'border-b border-gray-100 dark:border-gray-800/50 hover:bg-blue-50/50 dark:hover:bg-blue-950/30 transition-colors duration-100 cursor-pointer group',
                idx % 2 === 1 && 'bg-gray-50/40 dark:bg-gray-800/20'
              )}
            >
              {row.getVisibleCells().map((cell) => {
                const align = (cell.column.columnDef.meta as { align?: string })?.align
                return (
                  <td
                    key={cell.id}
                    className={cn(
                      'py-1.5 px-3 overflow-hidden',
                      align === 'right' && 'text-right tabular-nums'
                    )}
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                )
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
