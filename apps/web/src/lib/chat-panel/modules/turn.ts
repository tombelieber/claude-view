import type { TurnState } from '../types'

export type TurnEvent =
  | { type: 'STREAM_DELTA' }
  | { type: 'BLOCKS_UPDATE' }
  | { type: 'TURN_COMPLETE' }
  | { type: 'TURN_ERROR' }
  | {
      type: 'PERMISSION_REQUEST'
      kind: 'permission' | 'question' | 'plan' | 'elicitation'
      requestId: string
    }
  | { type: 'SESSION_COMPACTING' }
  | { type: 'COMPACT_DONE' }

export function turnTransition(state: TurnState, event: TurnEvent): TurnState {
  switch (state.turn) {
    case 'idle':
      switch (event.type) {
        case 'STREAM_DELTA':
        case 'BLOCKS_UPDATE':
          return { turn: 'streaming' }
        case 'PERMISSION_REQUEST':
          return { turn: 'awaiting', kind: event.kind, requestId: event.requestId }
        case 'SESSION_COMPACTING':
          return { turn: 'compacting' }
        default:
          return state
      }

    case 'streaming':
      switch (event.type) {
        case 'TURN_COMPLETE':
        case 'TURN_ERROR':
          return { turn: 'idle' }
        case 'PERMISSION_REQUEST':
          return { turn: 'awaiting', kind: event.kind, requestId: event.requestId }
        default:
          return state
      }

    case 'awaiting':
      switch (event.type) {
        case 'TURN_COMPLETE':
        case 'TURN_ERROR':
          return { turn: 'idle' }
        case 'STREAM_DELTA':
          return { turn: 'streaming' }
        default:
          return state
      }

    case 'compacting':
      switch (event.type) {
        case 'COMPACT_DONE':
          return { turn: 'idle' }
        default:
          return state
      }
  }
}
