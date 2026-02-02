import { useState } from 'react'
import { Archive, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'

interface FileSnapshotCardProps {
  fileCount: number
  timestamp: string
  files: string[]
  isIncremental: boolean
}

export function FileSnapshotCard({
  fileCount,
  timestamp,
  files,
  isIncremental,
}: FileSnapshotCardProps) {
  const isEmpty = fileCount === 0 && files.length === 0
  const defaultExpanded = files.length > 0 && files.length <= 10
  const [expanded, setExpanded] = useState(defaultExpanded)

  if (isEmpty) {
    return (
      <div
        className={cn(
          'rounded-lg border border-gray-200 dark:border-gray-700 border-l-4 border-l-blue-300 dark:border-l-blue-500 bg-blue-50 dark:bg-blue-950/30 p-3 my-2'
        )}
        aria-label="File snapshot"
      >
        <div className="flex items-start gap-2">
          <Archive
            className="w-4 h-4 text-blue-500 mt-0.5 flex-shrink-0"
            aria-hidden="true"
          />
          <div className="text-sm text-blue-700 dark:text-blue-300">
            Empty snapshot — No files
          </div>
        </div>
      </div>
    )
  }

  const headerText = `${fileCount} file${fileCount !== 1 ? 's' : ''} backed up at ${timestamp}`

  return (
    <div
      className={cn(
        'rounded-lg border border-gray-200 dark:border-gray-700 border-l-4 border-l-blue-300 dark:border-l-blue-500 overflow-hidden bg-white dark:bg-gray-900 my-2'
      )}
      aria-label="File snapshot"
    >
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left bg-blue-50 dark:bg-blue-950/30 hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-colors"
        aria-expanded={expanded}
      >
        <Archive
          className="w-4 h-4 text-blue-500 flex-shrink-0"
          aria-hidden="true"
        />
        <span className="text-sm text-blue-700 dark:text-blue-300 flex-1">
          {headerText}
          {isIncremental && (
            <span className="ml-2 text-xs text-blue-500 font-medium">
              (incremental)
            </span>
          )}
        </span>
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-blue-400" />
        ) : (
          <ChevronRight className="w-4 h-4 text-blue-400" />
        )}
      </button>
      {expanded && (
        <div className="px-3 py-2 border-t border-blue-100 dark:border-blue-800 bg-white dark:bg-gray-900">
          <ul className="text-xs text-gray-600 dark:text-gray-400 space-y-0.5">
            {files.map((file, i) => (
              <li key={i} className="font-mono truncate">
                {file}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}
