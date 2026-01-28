import { useState } from 'react'
import { FileText, ChevronDown, ChevronUp, Eye, Pencil, AlertTriangle } from 'lucide-react'
import { cn } from '../lib/utils'

export interface FileTouched {
  /** File path */
  path: string
  /** Number of times this file was read */
  readCount: number
  /** Number of times this file was edited */
  editCount: number
  /** Whether this file was re-edited (edited more than once) */
  isReedited: boolean
}

export interface FilesTouchedPanelProps {
  /** List of files with their access counts */
  files: FileTouched[]
  /** Number of files to show before "Show more" (default: 5) */
  initialLimit?: number
  /** Optional className for additional styling */
  className?: string
}

/**
 * Build FileTouched array from separate read/edit arrays.
 * Utility function to construct the data structure from SessionDetail fields.
 */
export function buildFilesTouched(
  filesRead: string[],
  filesEdited: string[],
  reeditedFiles?: Set<string>
): FileTouched[] {
  const fileMap = new Map<string, FileTouched>()

  // Count reads
  for (const path of filesRead) {
    const existing = fileMap.get(path)
    if (existing) {
      existing.readCount++
    } else {
      fileMap.set(path, {
        path,
        readCount: 1,
        editCount: 0,
        isReedited: false,
      })
    }
  }

  // Count edits
  for (const path of filesEdited) {
    const existing = fileMap.get(path)
    if (existing) {
      existing.editCount++
    } else {
      fileMap.set(path, {
        path,
        readCount: 0,
        editCount: 1,
        isReedited: false,
      })
    }
  }

  // Mark re-edited files
  if (reeditedFiles) {
    for (const path of reeditedFiles) {
      const file = fileMap.get(path)
      if (file) {
        file.isReedited = true
      }
    }
  } else {
    // If no explicit set provided, mark files with editCount > 1 as re-edited
    for (const file of fileMap.values()) {
      if (file.editCount > 1) {
        file.isReedited = true
      }
    }
  }

  // Sort: re-edited first, then by edit count, then by read count
  return Array.from(fileMap.values()).sort((a, b) => {
    if (a.isReedited !== b.isReedited) return a.isReedited ? -1 : 1
    if (a.editCount !== b.editCount) return b.editCount - a.editCount
    return b.readCount - a.readCount
  })
}

/** Get just the filename from a path */
function getFileName(path: string): string {
  const parts = path.split('/')
  return parts[parts.length - 1] || path
}

/** Get parent directory path for display */
function getParentDir(path: string): string {
  const parts = path.split('/')
  if (parts.length <= 1) return ''
  return parts.slice(0, -1).join('/')
}

/**
 * FilesTouchedPanel displays a list of files with read/edit counts.
 *
 * Features:
 * - Shows file path with read/edit counts
 * - Highlights re-edited files with [!] badge
 * - Expandable for long lists (default limit: 5)
 * - Sorted by: re-edited first, then edit count, then read count
 */
export function FilesTouchedPanel({
  files,
  initialLimit = 5,
  className,
}: FilesTouchedPanelProps) {
  const [isExpanded, setIsExpanded] = useState(false)

  const displayFiles = isExpanded ? files : files.slice(0, initialLimit)
  const hasMore = files.length > initialLimit

  if (files.length === 0) {
    return (
      <div className={cn('bg-white rounded-xl border border-gray-200 p-6', className)}>
        <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5 font-metric-label">
          <FileText className="w-4 h-4" />
          Files Touched
        </h2>
        <div className="flex flex-col items-center justify-center py-6 text-gray-400">
          <FileText className="w-8 h-8 mb-2 opacity-50" />
          <p className="text-sm">No files touched</p>
        </div>
      </div>
    )
  }

  return (
    <div className={cn('bg-white rounded-xl border border-gray-200 p-6', className)}>
      <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5 font-metric-label">
        <FileText className="w-4 h-4" />
        Files Touched
        <span className="ml-auto text-gray-400 normal-case font-normal">
          {files.length} {files.length === 1 ? 'file' : 'files'}
        </span>
      </h2>
      <div className="space-y-1">
        {displayFiles.map((file) => (
          <div
            key={file.path}
            className={cn(
              'flex items-center gap-2 p-2 -mx-2 rounded-lg transition-colors',
              file.isReedited ? 'bg-amber-50' : 'hover:bg-gray-50'
            )}
          >
            {/* Re-edit badge */}
            {file.isReedited && (
              <span
                className="flex items-center justify-center w-5 h-5 rounded bg-amber-500 text-white flex-shrink-0"
                title="This file was re-edited (multiple edits)"
              >
                <AlertTriangle className="w-3 h-3" />
              </span>
            )}

            {/* File path */}
            <div className="flex-1 min-w-0">
              <p className="text-sm text-gray-900 font-medium truncate font-mono">
                {getFileName(file.path)}
              </p>
              {getParentDir(file.path) && (
                <p className="text-xs text-gray-400 truncate">
                  {getParentDir(file.path)}
                </p>
              )}
            </div>

            {/* Counts */}
            <div className="flex items-center gap-2 flex-shrink-0">
              {file.readCount > 0 && (
                <span
                  className="flex items-center gap-1 text-xs text-gray-500"
                  title={`Read ${file.readCount} time${file.readCount > 1 ? 's' : ''}`}
                >
                  <Eye className="w-3 h-3" />
                  <span className="font-metric-value tabular-nums">{file.readCount}</span>
                </span>
              )}
              {file.editCount > 0 && (
                <span
                  className={cn(
                    'flex items-center gap-1 text-xs',
                    file.isReedited ? 'text-amber-600' : 'text-gray-500'
                  )}
                  title={`Edited ${file.editCount} time${file.editCount > 1 ? 's' : ''}`}
                >
                  <Pencil className="w-3 h-3" />
                  <span className="font-metric-value tabular-nums">{file.editCount}</span>
                </span>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Show more/less button */}
      {hasMore && (
        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className="mt-3 flex items-center justify-center gap-1 w-full py-2 text-sm text-gray-500 hover:text-gray-700 hover:bg-gray-50 rounded-lg transition-colors"
        >
          {isExpanded ? (
            <>
              <ChevronUp className="w-4 h-4" />
              Show less
            </>
          ) : (
            <>
              <ChevronDown className="w-4 h-4" />
              Show {files.length - initialLimit} more
            </>
          )}
        </button>
      )}
    </div>
  )
}
