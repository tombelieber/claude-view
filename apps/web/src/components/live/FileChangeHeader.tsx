import { ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { useFileDiff } from '../../hooks/use-file-history'
import { cn } from '../../lib/utils'
import type { FileChange } from '../../types/generated/FileChange'
import { DiffViewer } from './DiffViewer'
import { VersionStepper } from './VersionStepper'

interface FileChangeHeaderProps {
  file: FileChange
  sessionId: string
}

export function FileChangeHeader({ file, sessionId }: FileChangeHeaderProps) {
  const [expanded, setExpanded] = useState(false)
  const maxVersion = file.versions.length > 0 ? Math.max(...file.versions.map((v) => v.version)) : 1
  const [fromVersion, setFromVersion] = useState(Math.max(1, maxVersion - 1))
  const [toVersion, setToVersion] = useState(maxVersion)

  // Only fetch diff for multi-version files; single-version files show "Initial version" message
  const shouldFetchDiff = expanded && maxVersion >= 2
  const { data: diff, isLoading } = useFileDiff(
    shouldFetchDiff ? sessionId : null,
    shouldFetchDiff ? file.fileHash : null,
    fromVersion,
    toVersion,
  )

  const Icon = expanded ? ChevronDown : ChevronRight

  return (
    <div
      className={cn(
        'rounded-lg overflow-hidden transition-colors duration-150',
        expanded ? 'border border-gray-200 dark:border-gray-800' : '',
      )}
    >
      {/* Clickable header */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className={cn(
          'flex items-center gap-2 w-full px-3 py-2 text-left transition-colors cursor-pointer',
          expanded
            ? 'bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-800'
            : 'hover:bg-gray-50 dark:hover:bg-gray-800/50 rounded-lg',
        )}
      >
        <Icon className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 flex-shrink-0" />
        <span className="text-xs font-mono text-gray-700 dark:text-gray-300 truncate flex-1 min-w-0">
          {file.filePath}
        </span>
        <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-indigo-50 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-400 flex-shrink-0">
          v{maxVersion}
        </span>
        {file.stats.added > 0 && (
          <span className="text-[10px] font-mono text-green-600 dark:text-green-400 flex-shrink-0">
            +{file.stats.added}
          </span>
        )}
        {file.stats.removed > 0 && (
          <span className="text-[10px] font-mono text-red-500 dark:text-red-400 flex-shrink-0">
            −{file.stats.removed}
          </span>
        )}
      </button>

      {/* Expanded content */}
      {expanded && (
        <>
          {maxVersion >= 2 && (
            <VersionStepper
              maxVersion={maxVersion}
              fromVersion={fromVersion}
              toVersion={toVersion}
              onSelect={(f, t) => {
                setFromVersion(f)
                setToVersion(t)
              }}
            />
          )}

          {isLoading && (
            <div className="p-3 space-y-2">
              {[75, 60, 85, 50].map((w) => (
                <div
                  key={w}
                  className="animate-pulse bg-gray-200 dark:bg-gray-800 rounded h-3"
                  style={{ width: `${w}%` }}
                />
              ))}
            </div>
          )}

          {diff && diff.hunks.length > 0 && <DiffViewer hunks={diff.hunks} />}

          {expanded && maxVersion === 1 && (
            <div className="p-3 text-xs text-gray-500 dark:text-gray-400 italic">
              Initial version — no previous version to diff against
            </div>
          )}
        </>
      )}
    </div>
  )
}
