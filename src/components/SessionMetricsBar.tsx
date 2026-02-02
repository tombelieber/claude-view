import { MessageSquare, Coins, FileText, RefreshCw, GitCommit } from 'lucide-react'
import { cn } from '../lib/utils'
import { formatNumber, formatPercent } from '../lib/format-utils'

export interface SessionMetricsBarProps {
  /** Number of user prompts */
  prompts: number
  /** Total tokens (input + output) */
  tokens: bigint | null
  /** Number of files read */
  filesRead: number
  /** Number of files edited */
  filesEdited: number
  /** Re-edit rate (0-1 or null if no files edited) */
  reeditRate: number | null
  /** Number of commits linked */
  commits: number
  /** Layout variant */
  variant?: 'horizontal' | 'vertical'
  /** Optional className for additional styling */
  className?: string
}

interface MetricItemProps {
  icon: React.ReactNode
  label: string
  value: string
  title?: string
  vertical?: boolean
}

function MetricItem({ icon, label, value, title, vertical }: MetricItemProps) {
  if (vertical) {
    return (
      <div
        className="flex items-center justify-between py-1.5"
        title={title}
        role="group"
        aria-label={`${label}: ${value}`}
      >
        <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400" aria-hidden="true">
          {icon}
          <span className="text-sm">{label}</span>
        </div>
        <span className="text-sm font-semibold text-gray-900 dark:text-gray-100 font-metric-value tabular-nums" aria-hidden="true">
          {value}
        </span>
        <span className="sr-only">{title}</span>
      </div>
    )
  }
  return (
    <div
      className="flex flex-col items-center gap-1"
      title={title}
      role="group"
      aria-label={`${label}: ${value}`}
    >
      <div className="flex items-center gap-1.5 text-gray-400 dark:text-gray-500" aria-hidden="true">
        {icon}
        <span className="text-xs font-metric-label">{label}</span>
      </div>
      <span className="text-sm font-semibold text-gray-900 dark:text-gray-100 font-metric-value tabular-nums" aria-hidden="true">
        {value}
      </span>
      <span className="sr-only">{title}</span>
    </div>
  )
}

/**
 * SessionMetricsBar displays 5 key metrics.
 *
 * Variants:
 * - horizontal (default): compact row with dividers, used in headers
 * - vertical: label-value rows, used in sidebar (plan B9.3)
 *
 * Metrics:
 * 1. Prompts - user prompt count
 * 2. Tokens - total tokens used
 * 3. Files Read / Files Edited
 * 4. Re-edit % - re-edit rate
 * 5. Commits - linked commit count
 */
export function SessionMetricsBar({
  prompts,
  tokens,
  filesRead,
  filesEdited,
  reeditRate,
  commits,
  variant = 'horizontal',
  className,
}: SessionMetricsBarProps) {
  const v = variant === 'vertical'

  if (v) {
    return (
      <div
        className={cn(
          'px-4 py-3 bg-gray-50 dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 divide-y divide-gray-100 dark:divide-gray-800',
          className
        )}
        aria-label="Session metrics"
      >
        <MetricItem
          icon={<MessageSquare className="w-3.5 h-3.5" />}
          label="Prompts"
          value={formatNumber(prompts)}
          title="Number of user prompts"
          vertical
        />
        <MetricItem
          icon={<Coins className="w-3.5 h-3.5" />}
          label="Tokens"
          value={formatNumber(tokens)}
          title="Total tokens (input + output)"
          vertical
        />
        <MetricItem
          icon={<FileText className="w-3.5 h-3.5" />}
          label="Files Read"
          value={formatNumber(filesRead)}
          title={`${filesRead} files read`}
          vertical
        />
        <MetricItem
          icon={<FileText className="w-3.5 h-3.5" />}
          label="Files Edited"
          value={formatNumber(filesEdited)}
          title={`${filesEdited} files edited`}
          vertical
        />
        <MetricItem
          icon={<RefreshCw className="w-3.5 h-3.5" />}
          label="Re-edits"
          value={formatPercent(reeditRate, true)}
          title="Percentage of edited files that were re-edited"
          vertical
        />
        <MetricItem
          icon={<GitCommit className="w-3.5 h-3.5" />}
          label="Commits"
          value={formatNumber(commits)}
          title="Number of commits linked to this session"
          vertical
        />
      </div>
    )
  }

  return (
    <div
      className={cn(
        'flex items-center justify-between gap-4 px-4 py-3 bg-gray-50 dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700',
        className
      )}
    >
      <MetricItem
        icon={<MessageSquare className="w-3.5 h-3.5" />}
        label="Prompts"
        value={formatNumber(prompts)}
        title="Number of user prompts"
      />
      <div className="w-px h-8 bg-gray-200 dark:bg-gray-700" />
      <MetricItem
        icon={<Coins className="w-3.5 h-3.5" />}
        label="Tokens"
        value={formatNumber(tokens)}
        title="Total tokens (input + output)"
      />
      <div className="w-px h-8 bg-gray-200 dark:bg-gray-700" />
      <MetricItem
        icon={<FileText className="w-3.5 h-3.5" />}
        label="Files"
        value={`${filesRead}R/${filesEdited}E`}
        title={`${filesRead} files read, ${filesEdited} files edited`}
      />
      <div className="w-px h-8 bg-gray-200 dark:bg-gray-700" />
      <MetricItem
        icon={<RefreshCw className="w-3.5 h-3.5" />}
        label="Re-edit"
        value={formatPercent(reeditRate, true)}
        title="Percentage of edited files that were re-edited"
      />
      <div className="w-px h-8 bg-gray-200 dark:bg-gray-700" />
      <MetricItem
        icon={<GitCommit className="w-3.5 h-3.5" />}
        label="Commits"
        value={formatNumber(commits)}
        title="Number of commits linked to this session"
      />
    </div>
  )
}
