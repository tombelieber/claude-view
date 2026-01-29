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
  /** Optional className for additional styling */
  className?: string
}

interface MetricItemProps {
  icon: React.ReactNode
  label: string
  value: string
  title?: string
}

function MetricItem({ icon, label, value, title }: MetricItemProps) {
  return (
    <div
      className="flex flex-col items-center gap-1"
      title={title}
    >
      <div className="flex items-center gap-1.5 text-gray-400">
        {icon}
        <span className="text-xs font-metric-label">{label}</span>
      </div>
      <span className="text-sm font-semibold text-gray-900 font-metric-value tabular-nums">
        {value}
      </span>
    </div>
  )
}

/**
 * SessionMetricsBar displays 5 key metrics in a horizontal layout.
 *
 * Metrics:
 * 1. Prompts - user prompt count
 * 2. Tokens - total tokens used
 * 3. Files (R/E) - files read / files edited
 * 4. Re-edit % - re-edit rate
 * 5. Commits - linked commit count
 *
 * Used in ConversationView sidebar.
 */
export function SessionMetricsBar({
  prompts,
  tokens,
  filesRead,
  filesEdited,
  reeditRate,
  commits,
  className,
}: SessionMetricsBarProps) {
  return (
    <div
      className={cn(
        'flex items-center justify-between gap-4 px-4 py-3 bg-gray-50 rounded-lg border border-gray-200',
        className
      )}
    >
      <MetricItem
        icon={<MessageSquare className="w-3.5 h-3.5" />}
        label="Prompts"
        value={formatNumber(prompts)}
        title="Number of user prompts"
      />
      <div className="w-px h-8 bg-gray-200" />
      <MetricItem
        icon={<Coins className="w-3.5 h-3.5" />}
        label="Tokens"
        value={formatNumber(tokens)}
        title="Total tokens (input + output)"
      />
      <div className="w-px h-8 bg-gray-200" />
      <MetricItem
        icon={<FileText className="w-3.5 h-3.5" />}
        label="Files"
        value={`${filesRead}R/${filesEdited}E`}
        title={`${filesRead} files read, ${filesEdited} files edited`}
      />
      <div className="w-px h-8 bg-gray-200" />
      <MetricItem
        icon={<RefreshCw className="w-3.5 h-3.5" />}
        label="Re-edit"
        value={formatPercent(reeditRate, true)}
        title="Percentage of edited files that were re-edited"
      />
      <div className="w-px h-8 bg-gray-200" />
      <MetricItem
        icon={<GitCommit className="w-3.5 h-3.5" />}
        label="Commits"
        value={formatNumber(commits)}
        title="Number of commits linked to this session"
      />
    </div>
  )
}
