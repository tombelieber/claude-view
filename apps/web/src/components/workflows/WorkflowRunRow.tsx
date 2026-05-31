import { ArrowRight, Workflow } from 'lucide-react'
import { Link } from 'react-router-dom'
import { cn } from '../../lib/utils'
import type { WorkflowRunSummary } from '../../types/generated/WorkflowRunSummary'

export type DateFilter = 'all' | 'day' | 'week' | 'month'

/** Shared column template so the table header and rows stay aligned. */
export const RUN_ROW_GRID =
  'grid grid-cols-[minmax(200px,1.6fr)_130px_120px_120px_110px_32px] gap-4'

const STATUS_STYLES: Record<string, string> = {
  completed:
    'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/40 dark:text-emerald-300 dark:border-emerald-900',
  running:
    'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-950/40 dark:text-blue-300 dark:border-blue-900',
  failed:
    'bg-red-50 text-red-700 border-red-200 dark:bg-red-950/40 dark:text-red-300 dark:border-red-900',
  unknown:
    'bg-gray-100 text-gray-700 border-gray-200 dark:bg-gray-900 dark:text-gray-300 dark:border-gray-800',
}

export function formatCompact(value: number): string {
  return new Intl.NumberFormat(undefined, { notation: 'compact' }).format(value)
}

export function runTimestamp(run: WorkflowRunSummary): number {
  return Number(run.startTime ?? run.updatedAt ?? 0)
}

function formatDate(value: number): string {
  if (!value) return 'Unknown'
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}

export function matchesDateFilter(run: WorkflowRunSummary, filter: DateFilter): boolean {
  if (filter === 'all') return true
  const timestamp = runTimestamp(run)
  if (!timestamp) return false
  const age = Date.now() - timestamp
  if (filter === 'day') return age <= 24 * 60 * 60 * 1000
  if (filter === 'week') return age <= 7 * 24 * 60 * 60 * 1000
  return age <= 30 * 24 * 60 * 60 * 1000
}

export function WorkflowRunRow({ run }: { run: WorkflowRunSummary }) {
  const statusClass = STATUS_STYLES[run.status] ?? STATUS_STYLES.unknown

  return (
    <Link
      to={`/workflows/runs/${encodeURIComponent(run.sessionId)}/${encodeURIComponent(run.runId)}`}
      className={cn(
        RUN_ROW_GRID,
        'items-center border-b border-gray-200 px-4 py-3 text-sm transition-colors hover:bg-gray-50 dark:border-gray-800 dark:hover:bg-gray-900/70',
      )}
    >
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <Workflow className="h-4 w-4 shrink-0 text-gray-500" />
          <span className="truncate font-medium text-gray-950 dark:text-gray-100">
            {run.workflowName}
          </span>
        </div>
        <div className="mt-1 flex min-w-0 items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
          <span className="truncate">{run.summary ?? run.runId}</span>
        </div>
      </div>
      <div className="min-w-0 text-xs text-gray-600 dark:text-gray-300">
        <div className="truncate">{run.projectDir}</div>
        <div className="mt-0.5 truncate text-gray-400">{run.sessionId}</div>
      </div>
      <div>
        <span className={cn('inline-flex rounded-md border px-2 py-1 text-xs', statusClass)}>
          {run.status}
        </span>
      </div>
      <div className="text-xs text-gray-600 dark:text-gray-300">
        {formatDate(runTimestamp(run))}
      </div>
      <div className="text-xs text-gray-600 dark:text-gray-300">
        <div>{formatCompact(Number(run.totalTokens))} tokens</div>
        <div className="mt-0.5 text-gray-400">{run.totalToolCalls} tools</div>
      </div>
      <ArrowRight className="h-4 w-4 text-gray-400" />
    </Link>
  )
}
