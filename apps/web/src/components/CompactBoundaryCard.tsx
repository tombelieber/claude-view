import { Scissors } from 'lucide-react'
import { formatNumber } from '../lib/format-utils'

interface CompactBoundaryCardProps {
  trigger: string
  preTokens: number
  postTokens?: number
}

export function CompactBoundaryCard({ trigger, preTokens, postTokens }: CompactBoundaryCardProps) {
  return (
    <div className="py-0.5 border-l-2 border-l-indigo-400 pl-1 my-1">
      <div className="flex items-center gap-1.5">
        <Scissors className="w-3 h-3 text-indigo-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
          Context compacted: {formatNumber(preTokens)}
          {postTokens !== undefined && (
            <> {'\u2192'} {formatNumber(postTokens)}</>
          )}
          {' '}tokens ({trigger})
        </span>
      </div>
    </div>
  )
}
