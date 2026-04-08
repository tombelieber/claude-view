import type { ConversationBlock, ProgressBlock } from '@claude-view/shared/types/blocks'

/** Extract a sortable timestamp from any ConversationBlock variant. */
export function blockTimestamp(b: ConversationBlock): number {
  if ('ts' in b) return b.ts
  if ('timestamp' in b && typeof b.timestamp === 'number') return b.timestamp
  return 0
}

/** Merge incoming hook blocks into existing blocks, dedup by ID, sort by timestamp. */
export function mergeHookBlocks(
  existing: ConversationBlock[],
  incoming: ConversationBlock[],
): ConversationBlock[] {
  const existingIds = new Set(existing.map((b) => b.id))
  const newHooks = incoming.filter((b) => !existingIds.has(b.id))
  return [...existing, ...newHooks].sort((a, b) => blockTimestamp(a) - blockTimestamp(b))
}

/** Insert a single block at the correct timestamp position. Replace if ID exists. */
export function insertBlockByTimestamp(
  blocks: ConversationBlock[],
  newBlock: ConversationBlock,
): ConversationBlock[] {
  // Replace existing block with same ID
  const existingIdx = blocks.findIndex((b) => b.id === newBlock.id)
  if (existingIdx >= 0) {
    return blocks.map((b, i) => (i === existingIdx ? newBlock : b))
  }
  // Insert at correct timestamp position (scan from end)
  const ts = blockTimestamp(newBlock)
  if (ts === 0) return [...blocks, newBlock] // No timestamp = append
  let i = blocks.length
  while (i > 0 && blockTimestamp(blocks[i - 1]) > ts) i--
  const result = [...blocks]
  result.splice(i, 0, newBlock)
  return result
}

/**
 * Fetch hook events from REST and convert to ConversationBlock[].
 *
 * Mirrors the Rust `make_hook_progress_block()` shape — ProgressBlock with
 * variant 'hook', category 'hook', data type 'hook'.
 */
export async function fetchHookEventBlocks(sessionId: string): Promise<ConversationBlock[]> {
  const r = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/hook-events`)
  if (!r.ok) throw new Error(`hook-events: ${r.status}`)
  const data = await r.json()
  const events: Record<string, unknown>[] = data.hookEvents ?? []

  return events.map((e, i): ProgressBlock => {
    const ts = e.timestamp as number
    const eventName = (e.eventName as string) ?? ''
    const toolName = e.toolName as string | undefined
    const label = (e.label as string) ?? ''

    return {
      type: 'progress',
      id: `hook-${ts}-${i}`,
      variant: 'hook',
      category: 'hook',
      data: {
        type: 'hook',
        hookEvent: eventName,
        hookName: toolName ?? eventName,
        command: '',
        statusMessage: label,
      },
      ts,
    }
  })
}
