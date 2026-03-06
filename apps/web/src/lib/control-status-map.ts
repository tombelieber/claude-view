import type { InputBarState } from '../components/chat/ChatInputBar'

/**
 * Map control session status string to InputBarState.
 * Shared by SessionDetailPanel (live monitor).
 *
 * Note: 'active' maps to 'streaming' (Claude is generating),
 * 'waiting_input' maps to 'active' (user can type).
 */
export function controlStatusToInputState(status: string | undefined): InputBarState {
  switch (status) {
    case 'active':
      return 'streaming'
    case 'waiting_input':
      return 'active'
    case 'waiting_permission':
      return 'waiting_permission'
    case 'connecting':
      return 'connecting'
    case 'reconnecting':
      return 'reconnecting'
    case 'completed':
      return 'completed'
    default:
      return 'dormant'
  }
}
