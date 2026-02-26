import { useState } from 'react'
import { BookOpen, ChevronRight, ChevronDown } from 'lucide-react'

interface SessionSummaryCardProps {
  summary: string
  leafUuid: string
  wordCount: number
  verboseMode?: boolean
}

const TRUNCATE_LENGTH = 150

export function SessionSummaryCard({
  summary,
  leafUuid,
  wordCount,
  verboseMode,
}: SessionSummaryCardProps) {
  const [expanded, setExpanded] = useState(verboseMode ?? false)

  if (!summary || summary.trim().length === 0) {
    return (
      <div
        className="py-0.5 border-l-2 border-l-gray-400 pl-1 my-1"
        aria-label="Session summary"
      >
        <div className="flex items-center gap-1.5">
          <BookOpen className="w-3 h-3 text-gray-500 dark:text-gray-400 flex-shrink-0" aria-hidden="true" />
          <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
            No summary available
          </span>
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
      className="py-0.5 border-l-2 border-l-gray-400 pl-1 my-1"
      aria-label="Session summary"
      data-leaf-uuid={leafUuid}
    >
      {/* Status line — clickable to expand */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-start gap-1.5 mb-0.5 w-full text-left"
        aria-expanded={expanded}
      >
        <BookOpen className="w-3 h-3 text-gray-500 dark:text-gray-400 mt-0.5 flex-shrink-0" aria-hidden="true" />
        <div className="flex-1 min-w-0">
          <div className="text-[10px] text-gray-600 dark:text-gray-300">
            <span className="font-mono font-medium">Session summary:</span>{' '}
            {displayText}
          </div>
          <div className="text-[9px] font-mono text-gray-400 dark:text-gray-500 mt-0.5">
            {wordCount} words
          </div>
        </div>
        {needsTruncation && (
          expanded ? (
            <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0 mt-0.5" />
          ) : (
            <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0 mt-0.5" />
          )
        )}
      </button>
    </div>
  )
}
