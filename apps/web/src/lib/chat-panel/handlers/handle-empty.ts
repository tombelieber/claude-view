import { outboxTransition } from '../modules/outbox'
import type { ChatPanelStore, PanelState, RawEvent, TransitionResult } from '../types'

export function handleEmpty(store: ChatPanelStore, event: RawEvent): TransitionResult {
  if (event.type === 'SEND_MESSAGE') {
    // Queue then mark sent — POST_CREATE carries it as initialMessage.
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
      sessionId: '',
      targetSessionId: null,
      action: 'create',
      historyBlocks: [],
      pendingMessage: event.text,
      step: { step: 'posting' },
    }
    return [
      {
        panel,
        outbox,
        meta: store.meta,
        projectPath: store.projectPath,
        lastModel: event.model ?? store.lastModel,
        lastPermissionMode: event.permissionMode ?? store.lastPermissionMode,
        historyPagination: null,
      },
      [
        {
          cmd: 'POST_CREATE',
          model: event.model ?? 'default',
          message: event.text,
          permissionMode: event.permissionMode,
          projectPath: store.projectPath ?? undefined,
        },
      ],
    ]
  }
  return [store, []]
}
