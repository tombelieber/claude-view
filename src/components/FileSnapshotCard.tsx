import { useState } from 'react'
import { Archive, ChevronRight, ChevronDown } from 'lucide-react'

interface FileSnapshotCardProps {
  fileCount: number
  timestamp: string
  files: string[]
  isIncremental: boolean
  verboseMode?: boolean
}

export function FileSnapshotCard({
  fileCount,
  timestamp,
  files,
  isIncremental,
  verboseMode,
}: FileSnapshotCardProps) {
  const isEmpty = fileCount === 0 && files.length === 0
  const defaultExpanded = verboseMode || (files.length > 0 && files.length <= 10)
  const [expanded, setExpanded] = useState(defaultExpanded)

  if (isEmpty) {
    return (
      <div
        className="py-0.5 border-l-2 border-l-blue-400 pl-1 my-1"
        aria-label="File snapshot"
      >
        <div className="flex items-center gap-1.5">
          <Archive className="w-3 h-3 text-blue-500 flex-shrink-0" aria-hidden="true" />
          <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
            Empty snapshot — No files
          </span>
        </div>
      </div>
    )
  }

  const headerText = `${fileCount} file${fileCount !== 1 ? 's' : ''} backed up at ${timestamp}`

  return (
    <div
      className="py-0.5 border-l-2 border-l-blue-400 pl-1 my-1"
      aria-label="File snapshot"
    >
      {/* Status line — clickable to expand */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 mb-0.5 w-full text-left"
        aria-expanded={expanded}
      >
        <Archive className="w-3 h-3 text-blue-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {headerText}
          {isIncremental && (
            <span className="ml-1 text-[9px] text-blue-500">(incremental)</span>
          )}
        </span>
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {/* Expanded file list */}
      {expanded && (
        <ul className="ml-4 mt-0.5 space-y-0.5">
          {files.map((file, i) => (
            <li key={i} className="text-[10px] font-mono text-gray-400 dark:text-gray-500 truncate">
              {file}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
