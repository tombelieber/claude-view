import { useState } from 'react'
import { Bookmark, ChevronRight, ChevronDown } from 'lucide-react'

interface SavedHookContextCardProps {
  content: unknown[]
  verboseMode?: boolean
}

export function SavedHookContextCard({ content, verboseMode }: SavedHookContextCardProps) {
  const [expanded, setExpanded] = useState(verboseMode ?? false)

  const count = content.length
  const firstItem = count > 0 ? String(content[0]).slice(0, 80) : ''
  const summary = count === 0
    ? 'Hook context (empty)'
    : count === 1
      ? `Hook context: ${firstItem}${String(content[0]).length > 80 ? '...' : ''}`
      : `Hook context: ${count} entries`

  return (
    <div className="py-0.5 border-l-2 border-l-emerald-400 pl-1 my-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 mb-0.5 w-full text-left"
        aria-expanded={expanded}
      >
        <Bookmark className="w-3 h-3 text-emerald-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {summary}
        </span>
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {expanded && count > 0 && (
        <ul className="ml-4 mt-0.5 space-y-0.5">
          {content.map((item, i) => (
            <li key={i} className="text-[10px] font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap break-all">
              {typeof item === 'string' ? item : JSON.stringify(item, null, 2)}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
