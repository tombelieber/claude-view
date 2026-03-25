import { cn } from '../../lib/utils'
import type { UserItemInfo } from '../../types/generated'
import { formatRelativeTime } from './format-helpers'

const KIND_LABEL: Record<string, string> = {
  skill: 'SKILL',
  command: 'CMD',
  agent: 'AGENT',
}

interface UserItemCardProps {
  item: UserItemInfo
}

export function UserItemCard({ item }: UserItemCardProps) {
  const invocations = Number(item.totalInvocations)
  const sessions = Number(item.sessionCount)
  const lastUsed = item.lastUsedAt ? formatRelativeTime(Number(item.lastUsedAt)) : null
  const kindLabel = KIND_LABEL[item.kind] ?? item.kind.toUpperCase()
  const isLongName = item.name.length > 24

  return (
    <button
      type="button"
      className={cn(
        'w-full text-left rounded-xl border px-4 py-3.5',
        'bg-white border-[rgba(88,86,214,0.2)]',
        'hover:border-[rgba(88,86,214,0.38)] hover:shadow-[0_3px_10px_rgba(0,0,0,0.08)]',
        'transition-all duration-150',
        'shadow-[0_1px_2px_rgba(0,0,0,0.04)]',
        // No muting — all cards always full opacity
      )}
    >
      {/* Row 1: name + kind badge + kebab */}
      <div className="flex items-center justify-between gap-2">
        <span
          className={cn(
            'font-semibold text-apple-text1 truncate min-w-0',
            isLongName ? 'text-xs' : 'text-sm',
          )}
        >
          {item.name}
        </span>
        <div className="flex items-center gap-1 flex-shrink-0">
          <span className="text-xs font-bold uppercase tracking-[0.05em] px-1.5 py-0.5 rounded-[5px] bg-apple-bg text-apple-text3 border border-apple-sep2">
            {kindLabel}
          </span>
          <span
            role="button"
            tabIndex={0}
            onClick={(e) => {
              e.stopPropagation()
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') e.stopPropagation()
            }}
            className="text-apple-text3 text-base cursor-pointer px-0.5 rounded hover:text-apple-text2 hover:bg-apple-bg transition-colors leading-none"
          >
            ···
          </span>
        </div>
      </div>

      {/* Row 2: path */}
      <div className="mt-1 text-xs text-apple-text3 font-mono truncate">{item.path}</div>

      {/* Row 3: usage */}
      <div className={cn('mt-1.5 text-xs text-apple-text3', invocations === 0 && 'italic')}>
        {invocations > 0
          ? `${invocations.toLocaleString()}× · ${sessions} session${sessions !== 1 ? 's' : ''}${lastUsed ? ` · ${lastUsed}` : ''}`
          : 'No usage in 30 days'}
      </div>
    </button>
  )
}
