import { useState } from 'react'
import { BookOpen, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'

interface SessionSummaryCardProps {
  summary: string
  leafUuid: string
  wordCount: number
}

const TRUNCATE_LENGTH = 150

export function SessionSummaryCard({
  summary,
  leafUuid,
  wordCount,
}: SessionSummaryCardProps) {
  const [expanded, setExpanded] = useState(false)

  if (!summary || summary.trim().length === 0) {
    return (
      <div
        className={cn(
          'rounded-lg border border-gray-200 dark:border-gray-700 border-l-4 border-l-gray-300 dark:border-l-gray-600 bg-gray-50 dark:bg-gray-900 p-3 my-2'
        )}
        aria-label="Session summary"
      >
        <div className="flex items-start gap-2">
          <BookOpen
            className="w-4 h-4 text-gray-500 dark:text-gray-400 mt-0.5 flex-shrink-0"
            aria-hidden="true"
          />
          <div className="text-sm text-gray-600 dark:text-gray-400">No summary available</div>
        </div>
      </div>
    )
  }

  const needsTruncation = summary.length > TRUNCATE_LENGTH
  const displayText = expanded || !needsTruncation
    ? summary
    : summary.slice(0, TRUNCATE_LENGTH) + '...'

  return (
    <div
      className={cn(
        'rounded-lg border border-gray-200 dark:border-gray-700 border-l-4 border-l-gray-300 dark:border-l-gray-600 overflow-hidden bg-white dark:bg-gray-900 my-2'
      )}
      aria-label="Session summary"
      data-leaf-uuid={leafUuid}
    >
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-start gap-2 px-3 py-2 text-left bg-gray-50 dark:bg-gray-800/50 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
        aria-expanded={expanded}
      >
        <BookOpen
          className="w-4 h-4 text-gray-500 dark:text-gray-400 mt-0.5 flex-shrink-0"
          aria-hidden="true"
        />
        <div className="flex-1 min-w-0">
          <div className="text-sm text-gray-800 dark:text-gray-200">
            <span className="font-medium">Session summary:</span>{' '}
            {displayText}
          </div>
          <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">
            {wordCount} words
          </div>
        </div>
        {needsTruncation && (
          expanded ? (
            <ChevronDown className="w-4 h-4 text-gray-400 dark:text-gray-500 flex-shrink-0 mt-0.5" />
          ) : (
            <ChevronRight className="w-4 h-4 text-gray-400 dark:text-gray-500 flex-shrink-0 mt-0.5" />
          )
        )}
      </button>
    </div>
  )
}
