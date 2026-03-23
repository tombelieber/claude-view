import type { ConversationBlock } from '@claude-view/shared/types/blocks'

/**
 * Content fingerprint for cross-source deduplication.
 *
 * History (Rust BlockAccumulator) and sidecar (TS StreamAccumulator) produce
 * different IDs for the same logical block:
 *   - User blocks: JSONL uuid vs `user-N` counter
 *   - Assistant blocks: `msg_*` Anthropic ID vs SDK-internal UUID
 *
 * Fingerprinting by content catches these duplicates so mergeBlocks
 * doesn't produce doubled user/assistant messages.
 */
function blockFingerprint(block: ConversationBlock): string | null {
  if (block.type === 'user') return `user:${block.text}`
  if (block.type === 'assistant' && block.segments) {
    for (const seg of block.segments) {
      if (seg.kind === 'text' && seg.text) return `assistant:${seg.text.slice(0, 200)}`
    }
  }
  return null
}

/**
 * Merge server blocks (from accumulator) with existing blocks (from FETCH_HISTORY).
 *
 * On resume, the sidecar accumulator starts fresh — it only has the current turn.
 * History blocks (from FETCH_HISTORY) are preserved by keeping any block whose ID
 * doesn't appear in the incoming set, then appending the incoming blocks.
 *
 * Cross-source deduplication: history and sidecar produce different IDs for the
 * same logical block. Content fingerprints (user text, assistant first segment)
 * catch these duplicates so the merge doesn't produce doubled messages.
 *
 * For new sessions: existing is empty → returns incoming unchanged.
 * For reconnects: accumulator has everything → history blocks deduplicated → no duplication.
 */
export function mergeBlocks(
  existing: ConversationBlock[],
  incoming: ConversationBlock[],
): ConversationBlock[] {
  if (existing.length === 0) return incoming

  const incomingIds = new Set(incoming.map((b) => b.id))
  const incomingFingerprints = new Set(
    incoming.map(blockFingerprint).filter((fp): fp is string => fp !== null),
  )

  const preserved = existing.filter((b) => {
    if (incomingIds.has(b.id)) return false
    const fp = blockFingerprint(b)
    if (fp && incomingFingerprints.has(fp)) return false
    return true
  })

  return [...preserved, ...incoming]
}
