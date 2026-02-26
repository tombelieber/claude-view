/**
 * Clean preview text by stripping XML tags, system prompt noise, quotes, and collapsing whitespace.
 * Extracted from SessionCard.tsx for shared use.
 */
export function cleanPreviewText(text: string): string {
  // Remove XML-like tags (including empty tags like <>)
  let cleaned = text.replace(/<[^>]*>/g, '')
  // Remove slash-command prefixes like "/superpowers:brainstorm"
  cleaned = cleaned.replace(/\/[\w-]+:[\w-]+\s*/g, '')
  // Remove "superpowers:" prefixed words
  cleaned = cleaned.replace(/superpowers:\S+\s*/g, '')
  // Unescape JSON escape sequences (raw JSONL content has literal \n, \\, \", \t)
  // Single-pass replacement to avoid \\t being mis-parsed as \t after \\ -> \ substitution
  // Use a Unicode private-use-area placeholder for literal backslash so it survives cleanup
  const BACKSLASH_PLACEHOLDER = '\uE000'
  cleaned = cleaned.replace(/\\(\\|n|r|t|")/g, (_match, ch) => {
    switch (ch) {
      case '\\':
        return BACKSLASH_PLACEHOLDER
      case 'n':
        return ' '
      case 'r':
        return ' '
      case 't':
        return ' '
      case '"':
        return '"'
      default:
        return ch
    }
  })
  // Strip any remaining lone backslashes before non-escape chars
  cleaned = cleaned.replace(/\\(?![\\nrt"])/g, '')
  // Restore literal backslashes from placeholder
  cleaned = cleaned.replaceAll(BACKSLASH_PLACEHOLDER, '\\')
  // Remove matching surrounding quotes (after escape processing so \" doesn't lose its quote)
  if (
    (cleaned.startsWith('"') && cleaned.endsWith('"')) ||
    (cleaned.startsWith("'") && cleaned.endsWith("'"))
  ) {
    cleaned = cleaned.slice(1, -1)
  }
  // Collapse whitespace
  cleaned = cleaned.replace(/\s+/g, ' ').trim()
  // If it starts with common system prompt patterns, show a clean label
  if (cleaned.startsWith('You are a ') || cleaned.startsWith('You are Claude')) {
    return 'System prompt session'
  }
  // If it looks like ls output or file listing
  if (cleaned.match(/^"?\s*total \d+/)) {
    return cleaned.slice(0, 100) + (cleaned.length > 100 ? '...' : '')
  }
  return cleaned
}

/**
 * Get a display title for a session by cascading through available title sources.
 *
 * Priority:
 * 1. Cleaned preview text (if non-empty after cleaning)
 * 2. Summary (if non-empty)
 * 3. 'Untitled session' fallback
 */
export function getSessionTitle(preview?: string, summary?: string | null): string {
  // Try preview first (with cleaning)
  if (preview) {
    const cleaned = cleanPreviewText(preview)
    if (cleaned) return cleaned
  }

  // Try summary
  if (summary && summary.trim()) {
    return summary.trim()
  }

  return 'Untitled session'
}
