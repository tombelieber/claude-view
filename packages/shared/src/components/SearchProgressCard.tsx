import { Search } from 'lucide-react'

interface SearchProgressCardProps {
  query: string
  resultCount: number
}

export function SearchProgressCard({ query, resultCount }: SearchProgressCardProps) {
  return (
    <div className="py-0.5 border-l-2 border-l-cyan-400 pl-1 my-1" aria-label="Search progress">
      <div className="flex items-center gap-1.5 mb-0.5">
        <Search className="w-3 h-3 text-cyan-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {query}
        </span>
        <span className="text-[10px] font-mono font-semibold tabular-nums px-1.5 py-0.5 rounded bg-cyan-500/10 dark:bg-cyan-500/20 text-cyan-700 dark:text-cyan-300 flex-shrink-0">
          {resultCount} {resultCount === 1 ? 'result' : 'results'}
        </span>
      </div>
    </div>
  )
}
