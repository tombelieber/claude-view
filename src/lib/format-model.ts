/**
 * Format a model ID into a human-readable name.
 *
 * Handles all patterns generically — no hardcoded model map:
 *   "claude-opus-4-6"             → "Claude Opus 4.6"
 *   "claude-opus-4-5-20251101"    → "Claude Opus 4.5"
 *   "claude-sonnet-4-20250514"    → "Claude Sonnet 4"
 *   "claude-3-5-sonnet-20241022"  → "Claude 3.5 Sonnet"
 *   "opus"                        → "Opus"  (bare alias)
 *   "gpt-4-turbo"                 → "gpt-4-turbo" (non-Claude passthrough)
 *
 * New models (5.0, 6.1, etc.) are handled automatically.
 */
export function formatModelName(modelId: string): string {
  if (!modelId) return modelId

  // Bare aliases: "opus", "sonnet", "haiku" → capitalize
  if (!modelId.includes('-')) {
    return modelId.charAt(0).toUpperCase() + modelId.slice(1)
  }

  // Non-Claude models pass through unchanged
  if (!modelId.startsWith('claude-')) return modelId

  // Need at least "claude-X-Y" (3 parts) to parse
  if (modelId.split('-').length < 3) return modelId

  // Pattern A: Modern — claude-{family}-{major}[-{minor}][-{date}]
  //   claude-opus-4-6, claude-sonnet-4-5-20250929, claude-opus-4-20250514
  //
  // IMPORTANT: minor uses (\d{1,2}), NOT (\d+). With (\d+), the greedy match
  // captures the 8-digit date suffix as a minor version number.
  // With (\d{1,2}), the regex engine backtracks correctly:
  //   "claude-opus-4-20250514" → minor skipped, date=20250514 → "Claude Opus 4"
  const modernMatch = modelId.match(
    /^claude-([a-z]+)-(\d+)(?:-(\d{1,2}))?(?:-(\d{8}))?$/
  )
  if (modernMatch) {
    const [, family, major, minor] = modernMatch
    const familyName = family.charAt(0).toUpperCase() + family.slice(1)
    const version = minor !== undefined ? `${major}.${minor}` : major
    return `Claude ${familyName} ${version}`
  }

  // Pattern B: Legacy — claude-{major}[-{minor}]-{family}[-{date}]
  //   claude-3-5-sonnet-20241022, claude-3-opus-20240229
  const legacyMatch = modelId.match(
    /^claude-(\d+)(?:-(\d{1,2}))?-([a-z]+)(?:-(\d{8}))?$/
  )
  if (legacyMatch) {
    const [, major, minor, family] = legacyMatch
    const familyName = family.charAt(0).toUpperCase() + family.slice(1)
    const version = minor !== undefined ? `${major}.${minor}` : major
    return `Claude ${version} ${familyName}`
  }

  // Pattern C: Unknown Claude format — strip date suffix, capitalize parts
  //   Handles multi-word families like "claude-3-super-fast-20260101"
  const parts = modelId.split('-')
  if (parts[parts.length - 1]?.match(/^\d{8}$/)) {
    parts.pop()
  }
  return parts
    .map((p, i) => (i === 0 ? 'Claude' : p.charAt(0).toUpperCase() + p.slice(1)))
    .join(' ')
}
