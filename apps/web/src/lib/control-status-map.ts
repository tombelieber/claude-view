import type { InputBarState } from '../components/chat/ChatInputBar'

/**
 * Map session state string (from useConversation / useSessionSource) to InputBarState.
 * Handles both legacy ControlStatus strings and new sessionState strings from useConversation.
 *
 * Note: 'active' maps to 'streaming' (Claude is generating),
 * 'waiting_input' maps to 'active' (user can type).
 */
export function deriveInputBarState(sessionState: string, isLive: boolean): InputBarState {
  if (!isLive) return 'dormant'
  switch (sessionState) {
    case 'waiting_input':
      return 'active'
    case 'active':
      return 'streaming'
    case 'waiting_permission':
      return 'waiting_permission'
    case 'compacting':
      return 'streaming'
    case 'initializing':
    case 'connecting':
      return 'connecting'
    case 'reconnecting':
      return 'reconnecting'
    case 'closed':
    case 'completed':
    case 'error':
    case 'fatal':
    case 'failed':
      return 'completed'
    default:
      return 'dormant'
  }
}
