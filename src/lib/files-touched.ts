/**
 * Utility for building FileTouched data structure from session fields.
 * Extracted from FilesTouchedPanel to enable Fast Refresh.
 */

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
