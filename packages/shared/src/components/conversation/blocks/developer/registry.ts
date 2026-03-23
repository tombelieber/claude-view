import type { BlockRenderer, BlockRenderers } from '../../types'
import { DevAssistantBlock } from './AssistantBlock'
import { DevInteractionBlock } from './InteractionBlock'
import { DevNoticeBlock } from './NoticeBlock'
import { DevProgressBlock } from './ProgressBlock'
import { DevSystemBlock } from './SystemBlock'
import { DevTurnBoundary } from './TurnBoundary'
import { DevUserBlock } from './UserBlock'

export const developerRegistry: BlockRenderers = {
  user: DevUserBlock as BlockRenderer,
  assistant: DevAssistantBlock as BlockRenderer,
  interaction: DevInteractionBlock as BlockRenderer,
  turn_boundary: DevTurnBoundary as BlockRenderer,
  notice: DevNoticeBlock as BlockRenderer,
  system: DevSystemBlock as BlockRenderer,
  progress: DevProgressBlock as BlockRenderer,
}
