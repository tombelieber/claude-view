import { outboxTransition } from '../modules/outbox'
import type { ChatPanelStore, PanelState, RawEvent, TransitionResult } from '../types'

export function handleClosed(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'closed') return [store, []]

  switch (event.type) {
    case 'SEND_MESSAGE': {
      // Queue then mark sent — POST_RESUME carries it as initialMessage.
      // Without initialMessage, SDK with prompt:bridge deadlocks.
      let outbox = outboxTransition(store.outbox, {
        type: 'QUEUE',
        localId: event.localId,
        text: event.text,
      })
      outbox = outboxTransition(outbox, {
        type: 'MARK_SENT',
        localId: event.localId,
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
      return [
        { panel, outbox, meta: store.meta },
        [
          {
            cmd: 'POST_RESUME',
            sessionId: p.sessionId,
            message: event.text,
            model: event.model,
            permissionMode: event.permissionMode,
          },
        ],
      ]
    }

    case 'LIVE_STATUS_CHANGED': {
      if (event.status === 'cc_owned') {
        const panel: PanelState = {
          phase: 'cc_cli',
          sessionId: p.sessionId,
          blocks: p.blocks,
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
