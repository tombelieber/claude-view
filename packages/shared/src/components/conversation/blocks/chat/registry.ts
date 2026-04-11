import type { SystemBlock } from '../../../../types/blocks'
import type { BlockRenderer, BlockRenderers } from '../../types'
import { ChatAssistantBlock } from './AssistantBlock'
import { ChatInteractionBlock } from './InteractionBlock'
import { ChatNoticeBlock } from './NoticeBlock'
import { ChatProgressBlock } from './ProgressBlock'
import { ChatSystemBlock, isChatVisibleQueueOp } from './SystemBlock'
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
    if (block.type === 'system' && block.variant === 'queue_operation') {
      return isChatVisibleQueueOp(block as SystemBlock)
    }
    // team_transcript has no renderer yet (A2 will add it)
    if (block.type === 'team_transcript') return false
    return true
  },
}
