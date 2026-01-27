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
 * Format: "fix-the-login-bug-974d98a2" (preview slug + UUID prefix)
 */
export function sessionSlug(preview: string, sessionId: string): string {
  const textPart = slugify(preview, 6)
  const idPart = sessionId.slice(0, 8)
  return `${textPart}-${idPart}`
}

/**
 * Extract the UUID prefix from a session slug.
 * The last 8 characters after the final hyphen group.
 */
export function extractSessionIdPrefix(slug: string): string {
  return slug.slice(-8)
}

/**
 * Generate a project slug from display name.
 */
export function projectSlug(displayName: string): string {
  return slugify(displayName, 10) || 'project'
}
