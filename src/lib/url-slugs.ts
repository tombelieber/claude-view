/**
 * Generate a URL-friendly slug from text.
 * Strips non-alphanumeric chars, lowercases, truncates to maxWords.
 */
export function slugify(text: string, maxWords = 6): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .trim()
    .split(/\s+/)
    .slice(0, maxWords)
    .join('-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    || 'session'
}

/**
 * Generate a human-readable session slug from preview text and session ID.
 * Format: "fix-the-login-bug--974d98a2-512b-41db-b6b6-4e865b4882de"
 * Uses "--" (double hyphen) as separator so the full session ID is preserved
 * and unambiguously extractable.
 */
export function sessionSlug(preview: string, sessionId: string): string {
  const textPart = slugify(preview, 6)
  return `${textPart}--${sessionId}`
}

/**
 * Extract the full session ID from a session slug.
 * The session ID follows the "--" separator.
 * Falls back to treating the entire slug as the ID if no separator found.
 */
export function sessionIdFromSlug(slug: string): string {
  const separator = slug.indexOf('--')
  if (separator === -1) return slug // Fallback: entire slug is the ID
  return slug.slice(separator + 2)
}

/**
 * Generate a project slug from display name.
 */
export function projectSlug(displayName: string): string {
  return slugify(displayName, 10) || 'project'
}
