import { Scissors } from 'lucide-react'
import { cn } from '../lib/utils'
import { formatTokenCount } from '../lib/format-utils'

interface CompactBoundaryCardProps {
  trigger: string
  preTokens: number
  postTokens?: number
}

export function CompactBoundaryCard({ trigger, preTokens, postTokens }: CompactBoundaryCardProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 my-2 px-3 py-2',
        'border-t border-b border-indigo-300 dark:border-indigo-700 bg-indigo-50/50 dark:bg-indigo-950/30'
      )}
    >
      <Scissors className="w-4 h-4 text-indigo-500 flex-shrink-0" aria-hidden="true" />
      <span className="text-sm text-indigo-700 dark:text-indigo-300">
        Context compacted: {formatTokenCount(preTokens)}
        {postTokens !== undefined && (
          <> {'\u2192'} {formatTokenCount(postTokens)}</>
        )}
        {' '}tokens ({trigger})
      </span>
    </div>
  )
}
