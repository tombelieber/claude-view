import { outboxTransition } from '../modules/outbox'
import type { ChatPanelStore, PanelState, RawEvent, TransitionResult } from '../types'

export function handleClosed(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'closed') return [store, []]

  switch (event.type) {
    case 'SEND_MESSAGE': {
      const outbox = outboxTransition(store.outbox, {
        type: 'QUEUE',
        localId: event.localId,
        text: event.text,
      })
      const panel: PanelState = {
        phase: 'acquiring',
        sessionId: p.sessionId,
        targetSessionId: null,
        action: 'resume',
        historyBlocks: p.blocks,
        pendingMessage: event.text,
        step: { step: 'posting' },
      }
      return [{ panel, outbox, meta: store.meta }, [{ cmd: 'POST_RESUME', sessionId: p.sessionId }]]
    }

    case 'LIVE_STATUS_CHANGED': {
      if (event.status === 'cc_owned') {
        const panel: PanelState = {
          phase: 'cc_cli',
          sessionId: p.sessionId,
          sub: { sub: 'watching' },
        }
        return [{ ...store, panel }, [{ cmd: 'OPEN_TERMINAL_WS', sessionId: p.sessionId }]]
      }
      return [store, []]
    }

    default:
      return [store, []]
  }
}
