import type { ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'

/**
 * Interleaves optimistic user blocks into stream blocks at the correct
 * turn positions. Each user message is placed before the first assistant
 * block of its corresponding turn.
 *
 * Turn detection: the first assistant/interaction block in the stream marks
 * turn 1. Each subsequent turn_boundary resets, so the next assistant block
 * marks the start of the next turn.
 */
export function interleaveUserBlocks(
  userBlocks: UserBlock[],
  streamBlocks: ConversationBlock[],
): ConversationBlock[] {
  if (userBlocks.length === 0) return streamBlocks
  if (streamBlocks.length === 0) return [...userBlocks]

  const result: ConversationBlock[] = []
  let userIdx = 0
  let turnStarted = false

  for (const block of streamBlocks) {
    // Before the first assistant-like block of each turn, insert the user message
    if (
      userIdx < userBlocks.length &&
      !turnStarted &&
      (block.type === 'assistant' || block.type === 'interaction')
    ) {
      result.push(userBlocks[userIdx++])
      turnStarted = true
    }

    result.push(block)

    // turn_boundary marks end of a turn — reset for next turn
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
