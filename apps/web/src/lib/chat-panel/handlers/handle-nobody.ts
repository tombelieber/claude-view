import { nobodyTransition } from '../modules/nobody'
import { outboxTransition } from '../modules/outbox'
import type { ChatPanelStore, PanelState, RawEvent, TransitionResult } from '../types'

export function handleNobody(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'nobody') return [store, []]

  switch (event.type) {
    case 'HISTORY_OK':
    case 'HISTORY_FAILED': {
      const sub = nobodyTransition(p.sub, event)
      return [{ ...store, panel: { ...p, sub } }, []]
    }

    case 'SIDECAR_NO_SESSION':
      return [store, []]

    case 'SIDECAR_HAS_SESSION': {
      const blocks = p.sub.sub === 'ready' ? p.sub.blocks : []
      const panel: PanelState = {
        phase: 'acquiring',
        sessionId: p.sessionId,
        targetSessionId: null,
        action: 'resume',
        historyBlocks: blocks,
        pendingMessage: null,
        step: { step: 'ws_connecting', controlId: event.controlId },
      }
      return [{ ...store, panel }, [{ cmd: 'OPEN_SIDECAR_WS', sessionId: p.sessionId }]]
    }

    case 'SEND_MESSAGE': {
      const outbox = outboxTransition(store.outbox, {
        type: 'QUEUE',
        localId: event.localId,
        text: event.text,
      })
      const blocks = p.sub.sub === 'ready' ? p.sub.blocks : []
      const panel: PanelState = {
        phase: 'acquiring',
        sessionId: p.sessionId,
        targetSessionId: null,
        action: 'resume',
        historyBlocks: blocks,
        pendingMessage: event.text,
        step: { step: 'posting' },
      }
      return [{ panel, outbox, meta: store.meta }, [{ cmd: 'POST_RESUME', sessionId: p.sessionId }]]
    }

    case 'FORK_SESSION': {
      const blocks = p.sub.sub === 'ready' ? p.sub.blocks : []
      const panel: PanelState = {
        phase: 'acquiring',
        sessionId: p.sessionId,
        targetSessionId: null,
        action: 'fork',
        historyBlocks: blocks,
        pendingMessage: event.message ?? null,
        step: { step: 'posting' },
      }
      return [
        { ...store, panel },
        [{ cmd: 'POST_FORK', sessionId: p.sessionId, message: event.message }],
      ]
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
