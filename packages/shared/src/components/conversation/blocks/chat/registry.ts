import type { SystemBlock } from '../../../../types/blocks'
import type { BlockRenderer, BlockRenderers } from '../../types'
import { ChatAssistantBlock } from './AssistantBlock'
import { ChatInteractionBlock } from './InteractionBlock'
import { ChatNoticeBlock } from './NoticeBlock'
import { ChatProgressBlock } from './ProgressBlock'
import { ChatSystemBlock, isChatVisibleQueueOp } from './SystemBlock'
import { ChatTeamTranscriptBlock } from './TeamTranscriptBlock'
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
  team_transcript: ChatTeamTranscriptBlock as BlockRenderer,
  canRender: (block) => {
    if (block.type === 'system' && block.variant === 'queue_operation') {
      return isChatVisibleQueueOp(block as SystemBlock)
    }
    return true
  },
}
