import { useMemo } from 'react'
import type { RichMessage } from '../components/live/RichPane'

export type DisplayItem =
  | { kind: 'message'; message: RichMessage }
  | { kind: 'tool_pair'; toolUse: RichMessage; toolResult: RichMessage | null }

/**
 * Pure pairing function (exported for testing without React).
 * Walks RichMessage[] and pairs each tool_use with its subsequent tool_result.
 * Thinking messages between a tool_use and its result are emitted as standalone
 * messages before the pair.
 */
export function pairMessages(messages: RichMessage[]): DisplayItem[] {
  const items: DisplayItem[] = []
  let i = 0

  while (i < messages.length) {
    const m = messages[i]

    if (m.type === 'tool_use') {
      const skipped: RichMessage[] = []
      let resultMsg: RichMessage | null = null
      let j = i + 1

      while (j < messages.length) {
        const next = messages[j]
        if (next.type === 'tool_result') {
          resultMsg = next
          break
        }
        if (next.type === 'thinking') {
          skipped.push(next)
          j++
          continue
        }
        break
      }

      for (const s of skipped) {
        items.push({ kind: 'message', message: s })
      }

      items.push({ kind: 'tool_pair', toolUse: m, toolResult: resultMsg })
      i = resultMsg ? j + 1 : (skipped.length > 0 ? j : i + 1)
      continue
    }

    items.push({ kind: 'message', message: m })
    i++
  }

  return items
}

/**
 * React hook that memoizes message pairing.
 */
export function usePairedMessages(messages: RichMessage[]): DisplayItem[] {
  return useMemo(() => pairMessages(messages), [messages])
}
