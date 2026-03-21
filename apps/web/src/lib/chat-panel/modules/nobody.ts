import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { NobodySub } from '../types'

export type NobodyEvent =
  | { type: 'HISTORY_OK'; blocks: ConversationBlock[] }
  | { type: 'HISTORY_FAILED' }

export function nobodyTransition(s: NobodySub, e: NobodyEvent): NobodySub {
  switch (e.type) {
    case 'HISTORY_OK':
      return { sub: 'ready', blocks: e.blocks }
    case 'HISTORY_FAILED':
      return { sub: 'ready', blocks: [] }
    default:
      return s
  }
}
