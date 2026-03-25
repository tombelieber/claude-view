import { Search } from 'lucide-react'

/**
 * SearchProgressCard — follows inline stats pattern.
 *
 * Schema: query, resultCount
 */

interface SearchProgressCardProps {
  query: string
  resultCount: number
}

export function SearchProgressCard({ query, resultCount }: SearchProgressCardProps) {
  return (
    <div className="flex items-center gap-2 text-xs font-mono" aria-label="Search progress">
      <Search className="w-3 h-3 text-cyan-500 flex-shrink-0" aria-hidden="true" />
      <span className="text-gray-700 dark:text-gray-300 truncate flex-1">{query}</span>
      <span className="font-semibold tabular-nums px-1.5 py-0.5 rounded bg-cyan-500/10 dark:bg-cyan-500/20 text-cyan-700 dark:text-cyan-300 flex-shrink-0">
        {resultCount} {resultCount === 1 ? 'result' : 'results'}
      </span>
    </div>
  )
}
