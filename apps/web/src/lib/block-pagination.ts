/** Maximum blocks per page for history pagination. */
export const BLOCK_PAGE_SIZE = 50

/**
 * Compute the initial page: load all if small, cap to 60% for large sessions.
 * Guarantees `offset > 0` for sessions with `total > BLOCK_PAGE_SIZE`, ensuring
 * scroll-up pagination is always available.
 */
export function computeInitialPage(total: number): { offset: number; size: number } {
  const maxInitial = Math.floor(total * 0.6)
  const size = total <= BLOCK_PAGE_SIZE ? total : Math.min(BLOCK_PAGE_SIZE, maxInitial)
  const offset = Math.max(0, total - size)
  return { offset, size }
}

/**
 * Compute the previous (older) page params with clamped limit to prevent overlap.
 * Returns undefined when already at the beginning (offset=0).
 */
export function computePreviousPage(
  currentOffset: number,
): { offset: number; limit: number } | undefined {
  if (currentOffset === 0) return undefined
  const prevOffset = Math.max(0, currentOffset - BLOCK_PAGE_SIZE)
  const limit = currentOffset - prevOffset
  return { offset: prevOffset, limit }
}
