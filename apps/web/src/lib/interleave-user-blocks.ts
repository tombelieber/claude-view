import type { ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'

/**
 * Interleaves optimistic user blocks into stream blocks at the correct
 * turn positions. Each user message is placed before the first assistant
 * block of its corresponding turn.
 *
 * Optimistic blocks are only placed AFTER the last turn_boundary in the
 * stream — they belong to the latest/new turn, not replayed previous turns.
 * Without this guard, a session resume replays all previous turns' events
 * and the optimistic message gets placed before the FIRST turn's response.
 */
export function interleaveUserBlocks(
  userBlocks: UserBlock[],
  streamBlocks: ConversationBlock[],
): ConversationBlock[] {
  if (userBlocks.length === 0) return streamBlocks
  if (streamBlocks.length === 0) return [...userBlocks]

  // Find the last turn_boundary — everything before it is completed/replayed turns.
  // Optimistic user messages belong AFTER it (in the latest turn).
  const lastBoundaryIdx = findLastIndex(streamBlocks, (b) => b.type === 'turn_boundary')

  const result: ConversationBlock[] = []
  let userIdx = 0
  let turnStarted = false

  for (let i = 0; i < streamBlocks.length; i++) {
    const block = streamBlocks[i]

    // Only interleave user messages after the last turn boundary
    if (
      i > lastBoundaryIdx &&
      userIdx < userBlocks.length &&
      !turnStarted &&
      (block.type === 'assistant' || block.type === 'interaction')
    ) {
      result.push(userBlocks[userIdx++])
      turnStarted = true
    }

    result.push(block)

    if (block.type === 'turn_boundary') {
      turnStarted = false
    }
  }

  // Remaining user messages (awaiting response or still streaming)
  while (userIdx < userBlocks.length) {
    result.push(userBlocks[userIdx++])
  }

  return result
}

/** Array.findLastIndex polyfill (not available in all targets). */
function findLastIndex<T>(arr: T[], pred: (item: T) => boolean): number {
  for (let i = arr.length - 1; i >= 0; i--) {
    if (pred(arr[i])) return i
  }
  return -1
}
