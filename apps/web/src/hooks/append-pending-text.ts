import type { AssistantBlock, ConversationBlock } from '@claude-view/shared/types/blocks'

/**
 * Pure function: appends pending streaming text to the last text segment
 * of the last assistant block. Returns the original array (referential identity)
 * if no append is needed.
 */
export function appendPendingText(
  blocks: ConversationBlock[],
  pendingText: string,
): ConversationBlock[] {
  if (!pendingText || blocks.length === 0) return blocks

  const last = blocks[blocks.length - 1]
  if (last.type !== 'assistant') return blocks

  const aBlock = last as AssistantBlock

  // Find the last text segment (manual loop -- no findLastIndex for ES2022 compat)
  let lastTextIdx = -1
  for (let i = aBlock.segments.length - 1; i >= 0; i--) {
    if (aBlock.segments[i].kind === 'text') {
      lastTextIdx = i
      break
    }
  }

  if (lastTextIdx === -1) {
    // No text segment exists -- create one at the end
    const newSegments = [...aBlock.segments, { kind: 'text' as const, text: pendingText }]
    return [...blocks.slice(0, -1), { ...aBlock, segments: newSegments }]
  }

  const seg = aBlock.segments[lastTextIdx] as {
    kind: 'text'
    text: string
    parentToolUseId?: string | null
  }
  const newSegments = [...aBlock.segments]
  newSegments[lastTextIdx] = { ...seg, text: seg.text + pendingText }
  return [...blocks.slice(0, -1), { ...aBlock, segments: newSegments }]
}
