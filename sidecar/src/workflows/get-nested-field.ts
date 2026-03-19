/**
 * Safely retrieves a value from a nested object using dot-separated path.
 * Returns undefined if any part of the path is missing or the input is null/undefined.
 * Supports array index access via numeric path segments (e.g. "items.1").
 */
export function getNestedField(obj: unknown, path: string): unknown {
  if (obj == null) return undefined

  const parts = path.split('.')
  let current: unknown = obj

  for (const part of parts) {
    if (current == null || typeof current !== 'object') return undefined
    current = (current as Record<string, unknown>)[part]
  }

  return current
}
