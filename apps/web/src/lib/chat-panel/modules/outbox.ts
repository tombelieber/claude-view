import type { OutboxState } from '../types'

export type OutboxEvent =
  | { type: 'QUEUE'; localId: string; text: string }
  | { type: 'MARK_SENT'; localId: string }
  | { type: 'MARK_FAILED'; localId: string }
  | { type: 'REMOVE'; localId: string }
  | { type: 'MARK_ALL_FAILED' }
  | { type: 'REMOVE_BY_TEXT'; text: string }

export function outboxTransition(state: OutboxState, event: OutboxEvent): OutboxState {
  switch (event.type) {
    case 'QUEUE':
      return {
        messages: [
          ...state.messages,
          { localId: event.localId, text: event.text, status: 'queued' },
        ],
      }

    case 'MARK_SENT': {
      let found = false
      const messages = state.messages.map((m) => {
        if (m.localId === event.localId) {
          found = true
          return { ...m, status: 'sent' as const, sentAt: Date.now() }
        }
        return m
      })
      return found ? { messages } : state
    }

    case 'MARK_FAILED': {
      let found = false
      const messages = state.messages.map((m) => {
        if (m.localId === event.localId) {
          found = true
          return { ...m, status: 'failed' as const }
        }
        return m
      })
      return found ? { messages } : state
    }

    case 'REMOVE':
      return { messages: state.messages.filter((m) => m.localId !== event.localId) }

    case 'MARK_ALL_FAILED':
      return {
        messages: state.messages.map((m) =>
          m.status === 'queued' || m.status === 'sent' ? { ...m, status: 'failed' as const } : m,
        ),
      }

    case 'REMOVE_BY_TEXT': {
      const idx = state.messages.findIndex((m) => m.text === event.text)
      if (idx === -1) return state
      return { messages: [...state.messages.slice(0, idx), ...state.messages.slice(idx + 1)] }
    }
  }
}
