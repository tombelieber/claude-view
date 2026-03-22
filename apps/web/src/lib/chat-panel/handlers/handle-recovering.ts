import { outboxTransition } from '../modules/outbox'
import type { ChatPanelStore, PanelState, RawEvent, TransitionResult } from '../types'

export function handleRecovering(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'recovering') return [store, []]

  switch (event.type) {
    case 'SEND_MESSAGE': {
      // Queue then immediately mark sent — POST_RESUME carries the message as initialMessage.
      // Without initialMessage, SDK with prompt:bridge (AsyncIterable) deadlocks waiting
      // for the first message and never emits session_init.
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
        { panel, outbox, meta: store.meta, projectPath: store.projectPath },
        [
          {
            cmd: 'POST_RESUME',
            sessionId: p.sessionId,
            message: event.text,
            model: event.model,
            permissionMode: event.permissionMode,
            projectPath: store.projectPath ?? undefined,
          },
        ],
      ]
    }

    // Late-arriving history response — update blocks if still empty
    case 'HISTORY_OK':
      if (p.blocks.length === 0) {
        return [{ ...store, panel: { ...p, blocks: event.blocks } }, []]
      }
      return [store, []]

    case 'LIVE_STATUS_CHANGED': {
      const updatedStore = event.projectPath ? { ...store, projectPath: event.projectPath } : store
      if (event.status === 'cc_owned') {
        const panel: PanelState = {
          phase: 'cc_cli',
          sessionId: p.sessionId,
          blocks: p.blocks,
          sub: { sub: 'watching' },
        }
        return [{ ...updatedStore, panel }, [{ cmd: 'OPEN_TERMINAL_WS', sessionId: p.sessionId }]]
      }
      return [updatedStore, []]
    }

    default:
      return [store, []]
  }
}
