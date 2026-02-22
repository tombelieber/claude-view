import type { Message } from '../types/generated'
import type { RichMessage } from '../components/live/RichPane'
import type { HookEventItem } from '../components/live/action-log/types'

/**
 * Convert hook events (from SQLite) into synthetic Message objects
 * compatible with the Chat view's MessageTyped component.
 *
 * These arrive with role "progress" and metadata.type "hook_event",
 * which triggers the HookEventRow rendering path in renderProgressSubtype().
 * The original HookEventItem is carried in metadata._hookEvent so the
 * existing HookEventRow component can be reused directly.
 */
export function hookEventsToMessages(events: HookEventItem[]): Message[] {
  return events.map((e) => ({
    uuid: `hook-event-${e.id}`,
    role: 'progress' as const,
    content: `Hook: ${e.eventName} — ${e.label}`,
    timestamp: e.timestamp > 0
      ? new Date(e.timestamp * 1000).toISOString()
      : null,
    metadata: {
      type: 'hook_event',
      _sortTs: e.timestamp > 0 ? e.timestamp : undefined,
      _hookEvent: e,
    },
    thinking: null,
    tool_calls: null,
    category: 'hook',
  }))
}

/**
 * Convert hook events (from SQLite) into RichMessage objects
 * compatible with the Rich view's ProgressMessageCard component.
 * Same as above — carries the original HookEventItem in metadata._hookEvent.
 */
export function hookEventsToRichMessages(events: HookEventItem[]): RichMessage[] {
  return events.map((e) => ({
    type: 'progress' as const,
    content: `Hook: ${e.eventName} — ${e.label}`,
    ts: e.timestamp > 0 ? e.timestamp : undefined,
    category: 'hook' as const,
    metadata: {
      type: 'hook_event',
      _hookEvent: e,
    },
  }))
}

/**
 * Extract a numeric timestamp (epoch seconds) from a Message for sorting.
 * Prefers metadata._sortTs (set by hookEventsToMessages) to avoid
 * re-parsing ISO strings.
 */
export function getMessageSortTs(m: Message): number | undefined {
  const fast = m.metadata?._sortTs
  if (typeof fast === 'number' && fast > 0) return fast
  if (!m.timestamp) return undefined
  const ms = Date.parse(m.timestamp)
  return !isNaN(ms) && ms > 0 ? ms / 1000 : undefined
}

/**
 * Merge two sorted-by-timestamp arrays into one, maintaining order.
 * Items without timestamps go at the end.
 *
 * Both inputs must already be sorted by the key returned by getTs.
 * hook_events from SQLite: ORDER BY timestamp ASC, id ASC
 * Messages from JSONL: chronological order from parser
 */
export function mergeByTimestamp<T>(
  a: T[],
  b: T[],
  getTs: (item: T) => number | undefined,
): T[] {
  if (b.length === 0) return a
  if (a.length === 0) return b

  const merged: T[] = []
  let ai = 0
  let bi = 0

  while (ai < a.length && bi < b.length) {
    const tsA = getTs(a[ai]) ?? Infinity
    const tsB = getTs(b[bi]) ?? Infinity
    if (tsA <= tsB) {
      merged.push(a[ai++])
    } else {
      merged.push(b[bi++])
    }
  }

  while (ai < a.length) merged.push(a[ai++])
  while (bi < b.length) merged.push(b[bi++])

  return merged
}
