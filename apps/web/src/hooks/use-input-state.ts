import { useMemo } from 'react'

export interface InputState {
  canSend: boolean
  disabled: boolean
  disabledReason: string
  placeholder: string
}

/** Pure function for testing without renderHook. */
export function computeInputState(sessionState: string, isLive: boolean): InputState {
  if (!isLive) {
    return {
      canSend: false,
      disabled: true,
      disabledReason: 'Resume to send messages',
      placeholder: '',
    }
  }

  switch (sessionState) {
    case 'waiting_input':
      return {
        canSend: true,
        disabled: false,
        disabledReason: '',
        placeholder: 'Message Claude...',
      }
    case 'active':
      return {
        canSend: false,
        disabled: true,
        disabledReason: 'Agent is processing...',
        placeholder: '',
      }
    case 'waiting_permission':
      return {
        canSend: false,
        disabled: true,
        disabledReason: 'Waiting for your response above',
        placeholder: '',
      }
    case 'compacting':
      return {
        canSend: false,
        disabled: true,
        disabledReason: 'Compacting context...',
        placeholder: '',
      }
    case 'initializing':
      return {
        canSend: false,
        disabled: true,
        disabledReason: 'Starting session...',
        placeholder: '',
      }
    case 'closed':
      return { canSend: false, disabled: true, disabledReason: 'Session ended', placeholder: '' }
    case 'error':
      return { canSend: false, disabled: true, disabledReason: 'Session error', placeholder: '' }
    default:
      return { canSend: false, disabled: true, disabledReason: '', placeholder: '' }
  }
}

export function useInputState(sessionState: string, isLive: boolean): InputState {
  return useMemo(() => computeInputState(sessionState, isLive), [sessionState, isLive])
}
