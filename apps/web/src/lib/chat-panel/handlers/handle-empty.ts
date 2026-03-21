import { outboxTransition } from '../modules/outbox'
import type { ChatPanelStore, PanelState, RawEvent, TransitionResult } from '../types'

export function handleEmpty(store: ChatPanelStore, event: RawEvent): TransitionResult {
  if (event.type === 'SEND_MESSAGE') {
    const outbox = outboxTransition(store.outbox, {
      type: 'QUEUE',
      localId: event.localId,
      text: event.text,
    })
    const panel: PanelState = {
      phase: 'acquiring',
      sessionId: '',
      targetSessionId: null,
      action: 'create',
      historyBlocks: [],
      pendingMessage: event.text,
      step: { step: 'posting' },
    }
    return [
      { panel, outbox, meta: store.meta },
      [{ cmd: 'POST_CREATE', model: 'default', message: event.text }],
    ]
  }
  return [store, []]
}
