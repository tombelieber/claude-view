import type { BlockRenderer, BlockRenderers } from '../../types'
import { ChatAssistantBlock } from './AssistantBlock'
import { ChatInteractionBlock } from './InteractionBlock'
import { ChatNoticeBlock } from './NoticeBlock'
import { ChatProgressBlock } from './ProgressBlock'
import { ChatSystemBlock } from './SystemBlock'
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
}
