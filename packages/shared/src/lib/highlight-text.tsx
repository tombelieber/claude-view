import type { ReactNode } from 'react'

/** Escape special regex characters in a string for safe use in `new RegExp()`. */
export function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/**
 * Highlight all occurrences of `query` in `text` with <mark> tags.
 * Returns plain string if no query or no matches; returns ReactNode[] if matches found.
 */
export function highlightText(text: string, query: string): ReactNode {
  if (!query.trim()) return text

  const escaped = escapeRegex(query)
  const regex = new RegExp(`(${escaped})`, 'gi')
  const parts = text.split(regex)

  // If only one part, no match was found
  if (parts.length === 1) return text

  // With a single capturing group, split places captured fragments at odd indices
  let offset = 0
  return parts.map((part, i) => {
    const key = `hl-${offset}`
    offset += part.length

    if (i % 2 === 1) {
      return (
        <mark
          key={key}
          className="bg-amber-200 dark:bg-amber-900/50 text-amber-900 dark:text-amber-200 rounded-sm"
        >
          {part}
        </mark>
      )
    }

    return part
  })
}
