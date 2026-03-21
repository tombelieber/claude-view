import { useMemo } from 'react'
import type { PanelMode } from '../lib/derive-panel-mode'

export interface InputState {
  canSend: boolean
  disabled: boolean
  disabledReason: string
  placeholder: string
}

/** Pure function for testing without renderHook. */
export function computeInputState(mode: PanelMode): InputState {
  switch (mode.mode) {
    case 'blank':
      return { canSend: false, disabled: true, disabledReason: '', placeholder: '' }
    case 'history':
      return {
        canSend: true,
        disabled: false,
        disabledReason: '',
        placeholder: 'Message Claude...',
      }
    case 'watching':
      return {
        canSend: true,
        disabled: false,
        disabledReason: '',
        placeholder: 'Send a message to take over...',
      }
    case 'connecting':
      return {
        canSend: false,
        disabled: true,
        disabledReason: mode.reason === 'initial' ? 'Starting session...' : 'Reconnecting...',
        placeholder: '',
      }
    case 'own':
      switch (mode.subState) {
        case 'active':
          return {
            canSend: true,
            disabled: false,
            disabledReason: '',
            placeholder: 'Message Claude...',
          }
        case 'streaming':
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
        default:
          return mode.subState satisfies never
      }
    case 'error':
      return {
        canSend: false,
        disabled: true,
        disabledReason: mode.reason === 'fatal' ? 'Session error' : 'Session replaced',
        placeholder: '',
      }
    default:
      return (mode as { mode: never }).mode satisfies never
  }
}

export function useInputState(mode: PanelMode): InputState {
  const modeKey = mode.mode
  const subKey = 'subState' in mode ? mode.subState : 'reason' in mode ? mode.reason : null
  return useMemo(() => computeInputState(mode), [modeKey, subKey])
}
