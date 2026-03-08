import { ChevronDown, ChevronRight, File, FileCode, FileJson, FileText } from 'lucide-react'
import { useState } from 'react'
import { useFileDiff } from '../../hooks/use-file-history'
import { cn } from '../../lib/utils'
import type { FileChange } from '../../types/generated/FileChange'
import { DiffViewer } from './DiffViewer'

const CODE_EXTS = new Set([
  'rs',
  'ts',
  'tsx',
  'js',
  'jsx',
  'py',
  'go',
  'rb',
  'java',
  'c',
  'cpp',
  'h',
  'cs',
  'swift',
  'kt',
  'scala',
  'zig',
  'vue',
  'svelte',
])
const CONFIG_EXTS = new Set(['json', 'yaml', 'yml', 'toml', 'xml', 'ini', 'env'])
const DOC_EXTS = new Set(['md', 'txt', 'rst', 'adoc'])

function getFileIcon(filePath: string) {
  const ext = filePath.split('.').pop()?.toLowerCase() || ''
  if (CODE_EXTS.has(ext)) return FileCode
  if (CONFIG_EXTS.has(ext)) return FileJson
  if (DOC_EXTS.has(ext)) return FileText
  return File
}

interface FileChangeHeaderProps {
  file: FileChange
  sessionId: string
}

export function FileChangeHeader({ file, sessionId }: FileChangeHeaderProps) {
  const [expanded, setExpanded] = useState(true)
  const maxVersion = file.versions.length > 0 ? Math.max(...file.versions.map((v) => v.version)) : 1
  // For single-version (new) files, diff from 0 (empty) to 1 to show full content
  const [fromVersion, setFromVersion] = useState(maxVersion === 1 ? 0 : Math.max(1, maxVersion - 1))
  const [toVersion, setToVersion] = useState(maxVersion)

  const shouldFetchDiff = expanded
  const {
    data: diff,
    isLoading,
    error: diffError,
  } = useFileDiff(
    shouldFetchDiff ? sessionId : null,
    shouldFetchDiff ? file.fileHash : null,
    fromVersion,
    toVersion,
    file.filePath,
  )

  const Icon = expanded ? ChevronDown : ChevronRight
  const FileIcon = getFileIcon(file.filePath)

  // Split path into directory + filename for visual hierarchy
  const parts = file.filePath.split('/')
  const basename = parts[parts.length - 1] || file.filePath
  const directory = parts.length > 1 ? `${parts.slice(0, -1).join('/')}/` : ''

  // Generate version pairs for inline stepper
  const versionPairs: [number, number][] = []
  for (let i = 1; i < maxVersion; i++) {
    versionPairs.push([i, i + 1])
  }

  return (
    <div
      className={cn(
        'rounded-lg overflow-hidden transition-colors duration-150',
        expanded ? 'border border-gray-200 dark:border-gray-800' : '',
      )}
    >
      {/* Single header row: chevron + icon + path + version pills + stats */}
      <div
        className={cn(
          'flex items-center gap-1.5 w-full px-3 py-1.5 transition-colors',
          expanded
            ? 'bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-800'
            : 'hover:bg-gray-50 dark:hover:bg-gray-800/50 rounded-lg',
        )}
      >
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 min-w-0 flex-1 cursor-pointer text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1 rounded"
        >
          <Icon className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 shrink-0" />
          <FileIcon className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 shrink-0" />
          <span className="text-xs font-mono truncate min-w-0">
            {directory && <span className="text-gray-400 dark:text-gray-500">{directory}</span>}
            <span className="text-gray-800 dark:text-gray-200 font-medium">{basename}</span>
          </span>
        </button>

        {/* Inline version stepper pills (multi-version files only) */}
        {maxVersion >= 2 && (
          <div className="flex items-center gap-0.5 shrink-0">
            {versionPairs.map(([f, t]) => {
              const isActive = f === fromVersion && t === toVersion
              return (
                <button
                  type="button"
                  key={`${f}-${t}`}
                  onClick={() => {
                    setFromVersion(f)
                    setToVersion(t)
                  }}
                  className={cn(
                    'text-[10px] font-mono px-1.5 py-0.5 rounded-full transition-colors cursor-pointer',
                    isActive
                      ? 'bg-indigo-500 text-white'
                      : 'bg-gray-100 dark:bg-gray-800 text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700',
                  )}
                >
                  v{f}→v{t}
                </button>
              )
            })}
          </div>
        )}

        {/* NEW badge for single-version files */}
        {maxVersion === 1 && (
          <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-green-50 dark:bg-green-900/30 text-green-600 dark:text-green-400 shrink-0">
            NEW
          </span>
        )}

        {/* Stats */}
        {file.stats.added > 0 && (
          <span className="text-[10px] font-mono text-green-600 dark:text-green-400 shrink-0">
            +{file.stats.added}
          </span>
        )}
        {file.stats.removed > 0 && (
          <span className="text-[10px] font-mono text-red-500 dark:text-red-400 shrink-0">
            −{file.stats.removed}
          </span>
        )}
      </div>

      {/* Diff content — directly below header, no extra toolbar */}
      {expanded && (
        <>
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

          {diffError && !isLoading && (
            <div className="p-3 text-xs text-red-500/70 dark:text-red-400/70">
              Failed to load diff
            </div>
          )}

          {diff && diff.hunks.length > 0 && <DiffViewer hunks={diff.hunks} />}

          {/* Fallback for new files when diff returned empty or failed */}
          {maxVersion === 1 && !isLoading && (!diff || diff.hunks.length === 0) && !diffError && (
            <div className="p-3 text-xs text-gray-500 dark:text-gray-400 italic flex items-center gap-2">
              <span>Initial version</span>
              {file.stats.added > 0 && (
                <span className="text-[10px] font-mono text-green-600 dark:text-green-400 not-italic">
                  {file.stats.added} lines
                </span>
              )}
            </div>
          )}
        </>
      )}
    </div>
  )
}
