import { useMemo } from 'react'

export interface InputState {
  canSend: boolean
  disabled: boolean
  disabledReason: string
  placeholder: string
}

/** Pure function for testing without renderHook. */
export function computeInputState(
  sessionState: string,
  isLive: boolean,
  canResumeLazy?: boolean,
): InputState {
  if (!isLive) {
    if (canResumeLazy) {
      // Lazy-resumable: user CAN type — WS will connect on first send
      return {
        canSend: true,
        disabled: false,
        disabledReason: '',
        placeholder: 'Message Claude...',
      }
    }
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

export function useInputState(
  sessionState: string,
  isLive: boolean,
  canResumeLazy?: boolean,
): InputState {
  return useMemo(
    () => computeInputState(sessionState, isLive, canResumeLazy),
    [sessionState, isLive, canResumeLazy],
  )
}
