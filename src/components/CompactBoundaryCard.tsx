import { Scissors } from 'lucide-react'
import { cn } from '../lib/utils'

interface CompactBoundaryCardProps {
  trigger: string
  preTokens: number
  postTokens?: number
}

function formatNumber(n: number): string {
  return n.toLocaleString('en-US')
}

export function CompactBoundaryCard({ trigger, preTokens, postTokens }: CompactBoundaryCardProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 my-2 px-3 py-2',
        'border-t border-b border-indigo-300 bg-indigo-50/50'
      )}
    >
      <Scissors className="w-4 h-4 text-indigo-500 flex-shrink-0" aria-hidden="true" />
      <span className="text-sm text-indigo-700">
        Context compacted: {formatNumber(preTokens)}
        {postTokens !== undefined && (
          <> {'\u2192'} {formatNumber(postTokens)}</>
        )}
        {' '}tokens ({trigger})
      </span>
    </div>
  )
}
