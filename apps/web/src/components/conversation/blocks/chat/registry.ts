import type { BlockRenderer, BlockRenderers } from '../../types'
import { ChatAssistantBlock } from './AssistantBlock'
import { ChatInteractionBlock } from './InteractionBlock'
import { ChatNoticeBlock } from './NoticeBlock'
import { ChatProgressBlock } from './ProgressBlock'
import { CHAT_SYSTEM_VARIANTS, ChatSystemBlock } from './SystemBlock'
import { ChatTurnBoundary } from './TurnBoundary'
import { ChatUserBlock } from './UserBlock'

export const chatRegistry: BlockRenderers = {
  user: ChatUserBlock as BlockRenderer,
  assistant: ChatAssistantBlock as BlockRenderer,
  interaction: ChatInteractionBlock as BlockRenderer,
  turn_boundary: ChatTurnBoundary as BlockRenderer,
  notice: ChatNoticeBlock as BlockRenderer,
  system: ChatSystemBlock as BlockRenderer,
  progress: ChatProgressBlock as BlockRenderer,
  canRender: (block) => {
    if (block.type === 'system' && 'variant' in block) {
      return CHAT_SYSTEM_VARIANTS.has(block.variant)
    }
    return true
  },
}
