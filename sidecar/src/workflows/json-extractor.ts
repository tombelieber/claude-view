/**
 * Extracts the last ```json ... ``` fenced code block from a text string.
 * Returns the trimmed content, or null if no block is found.
 */
export function extractLastJsonBlock(text: string): string | null {
  const matches = [...text.matchAll(/```json\s*\n([\s\S]*?)\n\s*```/g)]
  if (matches.length === 0) return null
  const lastMatch = matches[matches.length - 1]
  return lastMatch[1].trim()
}
