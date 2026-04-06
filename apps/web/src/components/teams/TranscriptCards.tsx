import Markdown from 'react-markdown'
import { markdownComponents } from '../../lib/markdown-components'
import { cn } from '../../lib/utils'

// ── Color mapping ────────────────────────────────────

const BORDER_COLOR_MAP: Record<string, string> = {
  blue: 'border-blue-500',
  green: 'border-green-500',
  yellow: 'border-yellow-500',
  purple: 'border-purple-500',
  red: 'border-red-500',
  orange: 'border-orange-500',
}

const DOT_COLOR_MAP: Record<string, string> = {
  blue: 'bg-blue-500',
  green: 'bg-green-500',
  yellow: 'bg-yellow-500',
  purple: 'bg-purple-500',
  red: 'bg-red-500',
  orange: 'bg-orange-500',
}

// ── AgentMessageCard ─────────────────────────────────

interface AgentMessageCardProps {
  teammateId: string
  displayName: string
  color?: string | null
  text: string
}

export function AgentMessageCard({ displayName, color, text }: AgentMessageCardProps) {
  const borderClass = BORDER_COLOR_MAP[color ?? ''] ?? 'border-gray-400'
  const dotClass = DOT_COLOR_MAP[color ?? ''] ?? 'bg-gray-400'

  return (
    <div className={cn('border-l-3 pl-4 py-3', borderClass)}>
      <div className="flex items-center gap-2 mb-2">
        <span className={cn('w-2.5 h-2.5 rounded-full shrink-0', dotClass)} />
        <span className="text-xs font-semibold text-gray-900 dark:text-gray-100">
          {displayName}
        </span>
      </div>
      <div className="prose prose-xs dark:prose-invert max-w-none text-gray-700 dark:text-gray-300">
        <Markdown components={markdownComponents}>{text}</Markdown>
      </div>
    </div>
  )
}

// ── ModeratorCard ────────────────────────────────────

interface ModeratorCardProps {
  text: string
}

export function ModeratorCard({ text }: ModeratorCardProps) {
  return (
    <div className="bg-zinc-50 dark:bg-zinc-800/50 rounded-lg px-4 py-3">
      <div className="text-xs font-medium text-zinc-500 dark:text-zinc-400 mb-1.5">Moderator</div>
      <div className="prose prose-xs dark:prose-invert max-w-none text-gray-700 dark:text-gray-300">
        <Markdown components={markdownComponents}>{text}</Markdown>
      </div>
    </div>
  )
}

// ── VerdictCard ──────────────────────────────────────

interface VerdictCardProps {
  text: string
}

export function VerdictCard({ text }: VerdictCardProps) {
  return (
    <div className="border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/50 rounded-lg px-4 py-3">
      <div className="text-xs font-semibold text-zinc-600 dark:text-zinc-300 uppercase tracking-wider mb-2">
        Verdict
      </div>
      <div className="prose prose-xs dark:prose-invert max-w-none text-gray-800 dark:text-gray-200 font-medium">
        <Markdown components={markdownComponents}>{text}</Markdown>
      </div>
    </div>
  )
}

// ── RoundDivider ─────────────────────────────────────

interface RoundDividerProps {
  label: string
}

export function RoundDivider({ label }: RoundDividerProps) {
  return (
    <div className="flex items-center gap-3 py-2">
      <div className="flex-1 h-px bg-zinc-200 dark:bg-zinc-700" />
      <span className="text-xs font-medium text-zinc-500 dark:text-zinc-400 whitespace-nowrap">
        {label}
      </span>
      <div className="flex-1 h-px bg-zinc-200 dark:bg-zinc-700" />
    </div>
  )
}
