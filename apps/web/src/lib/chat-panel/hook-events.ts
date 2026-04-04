import type { ConversationBlock, ProgressBlock } from '@claude-view/shared/types/blocks'

/** Extract a sortable timestamp from any ConversationBlock variant. */
export function blockTimestamp(b: ConversationBlock): number {
  if ('ts' in b) return b.ts
  if ('timestamp' in b && typeof b.timestamp === 'number') return b.timestamp
  return 0
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
