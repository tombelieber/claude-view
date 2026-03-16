import type { InputBarState } from '../components/chat/ChatInputBar'

/**
 * Map session state string (from useConversation / useSessionSource) to InputBarState.
 * Handles both legacy ControlStatus strings and new sessionState strings from useConversation.
 *
 * Note: 'active' maps to 'streaming' (Claude is generating),
 * 'waiting_input' maps to 'active' (user can type).
 */
export function deriveInputBarState(
  sessionState: string,
  isLive: boolean,
  canResumeLazy?: boolean,
  isSpectating?: boolean,
): InputBarState {
  // Spectating a live session controlled by another process — hard block all input
  if (isSpectating) return 'controlled_elsewhere'

  // Connection replaced by another tab — show as completed (not an error)
  if (sessionState === 'replaced') return 'completed'

  if (!isLive) {
    return canResumeLazy ? 'active' : 'dormant'
  }
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
