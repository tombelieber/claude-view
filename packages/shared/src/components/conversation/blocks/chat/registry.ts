import type {
  AssistantBlock,
  ConversationBlock,
  ProgressBlock,
  SystemBlock,
} from '../../../../types/blocks'
import type { BlockRenderer, BlockRenderers } from '../../types'
import { ChatAssistantBlock, isChatAssistantBlockEmpty } from './AssistantBlock'
import { ChatInteractionBlock } from './InteractionBlock'
import { ChatNoticeBlock } from './NoticeBlock'
import { ChatProgressBlock } from './ProgressBlock'
import { ChatSystemBlock } from './SystemBlock'
import { ChatTeamTranscriptBlock } from './TeamTranscriptBlock'
import { ChatTurnBoundary } from './TurnBoundary'
import { ChatUserBlock } from './UserBlock'

/**
 * Whether a block should render in chat mode at all.
 *
 * This lives at the registry level (rather than returning null inside each
 * block renderer) because `ConversationThread` wraps every item in a padded
 * `<div className="py-1.5 …">` BEFORE calling the renderer — so a null-returning
 * renderer still leaves a visible 12px gap in the list. Pre-filtering here
 * removes the item entirely, keeping chat mode tight.
 *
 * Rules encoded:
 *   - Empty assistant blocks (no segments, no thinking, not streaming) — the
 *     stream accumulator commits these when a turn aborts before content; the
 *     only thing they'd render is a bare MessageTimestamp row.
 *   - Hook progress blocks (ProgressBlock with variant='hook') — one per hook
 *     invocation × 100+ tools per session = dominant source of chat noise.
 *   - Hook event system blocks (SystemBlock with variant='hook_event') — the
 *     WebSocket-channel twin of the REST hook progress; same noise story.
 *
 * Developer mode bypasses this entirely — `developerRegistry` has no
 * `canRender`, so every block reaches `DevAssistantBlock` / `DevProgressBlock`
 * / `DevSystemBlock`. Zero data loss for the debug tool.
 */
function canRenderInChat(block: ConversationBlock): boolean {
  if (block.type === 'assistant') {
    return !isChatAssistantBlockEmpty(block as AssistantBlock)
  }
  if (block.type === 'progress' && (block as ProgressBlock).variant === 'hook') {
    return false
  }
  if (block.type === 'system' && (block as SystemBlock).variant === 'hook_event') {
    return false
  }
  return true
}

export const chatRegistry: BlockRenderers = {
  user: ChatUserBlock as BlockRenderer,
  assistant: ChatAssistantBlock as BlockRenderer,
  interaction: ChatInteractionBlock as BlockRenderer,
  turn_boundary: ChatTurnBoundary as BlockRenderer,
  notice: ChatNoticeBlock as BlockRenderer,
  system: ChatSystemBlock as BlockRenderer,
  progress: ChatProgressBlock as BlockRenderer,
  team_transcript: ChatTeamTranscriptBlock as BlockRenderer,
  canRender: canRenderInChat,
}
