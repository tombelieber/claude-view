import { nobodyTransition } from '../modules/nobody'
import { outboxTransition } from '../modules/outbox'
import type {
  ChatPanelStore,
  HistoryPagination,
  PanelState,
  RawEvent,
  TransitionResult,
} from '../types'

export function handleNobody(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'nobody') return [store, []]

  switch (event.type) {
    case 'HISTORY_OK':
    case 'HISTORY_FAILED': {
      const sub = nobodyTransition(p.sub, event)
      // Store pagination metadata from HISTORY_OK
      let pagination: HistoryPagination | null = store.historyPagination
      if (event.type === 'HISTORY_OK' && event.total != null && event.offset != null) {
        pagination = { total: event.total, offset: event.offset, fetchingOlder: false }
      }
      // If LIVE_STATUS_CHANGED(cc_owned) arrived while loading, now that
      // history is ready, complete the deferred transition to cc_cli.
      if (sub.sub === 'ready' && p.sub.sub === 'loading' && p.sub.pendingLive === 'cc_owned') {
        const panel: PanelState = {
          phase: 'cc_cli',
          sessionId: p.sessionId,
          blocks: sub.blocks,
          sub: { sub: 'watching' },
        }
        return [
          { ...store, panel, historyPagination: pagination },
          [{ cmd: 'OPEN_TERMINAL_WS', sessionId: p.sessionId }],
        ]
      }
      return [{ ...store, panel: { ...p, sub }, historyPagination: pagination }, []]
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
      // Queue then immediately mark sent — POST_RESUME carries the message as initialMessage.
      // Keeping it in outbox (as 'sent') preserves the optimistic UI block via reconcileOutbox.
      // Status 'sent' prevents exitAcquiring from draining it again (only drains 'queued').
      let outbox = outboxTransition(store.outbox, {
        type: 'QUEUE',
        localId: event.localId,
        text: event.text,
      })
      outbox = outboxTransition(outbox, {
        type: 'MARK_SENT',
        localId: event.localId,
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
      return [
        {
          panel,
          outbox,
          meta: store.meta,
          projectPath: store.projectPath,
          lastModel: event.model ?? store.lastModel,
          lastPermissionMode: event.permissionMode ?? store.lastPermissionMode,
          historyPagination: store.historyPagination,
        },
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
        [
          {
            cmd: 'POST_FORK',
            sessionId: p.sessionId,
            message: event.message,
            projectPath: store.projectPath ?? undefined,
          },
        ],
      ]
    }

    case 'LIVE_STATUS_CHANGED': {
      // Update projectPath from Live Monitor data whenever available
      const updatedStore = event.projectPath ? { ...store, projectPath: event.projectPath } : store
      if (event.status === 'cc_owned') {
        if (p.sub.sub === 'ready') {
          // History loaded — transition immediately with blocks
          const panel: PanelState = {
            phase: 'cc_cli',
            sessionId: p.sessionId,
            blocks: p.sub.blocks,
            sub: { sub: 'watching' },
          }
          return [{ ...updatedStore, panel }, [{ cmd: 'OPEN_TERMINAL_WS', sessionId: p.sessionId }]]
        }
        // History still loading — defer cc_cli transition until HISTORY_OK arrives.
        // Without this, cc_cli starts with blocks: [] and the user sees a blank page.
        return [
          { ...updatedStore, panel: { ...p, sub: { sub: 'loading', pendingLive: 'cc_owned' } } },
          [],
        ]
      }
      return [updatedStore, []]
    }

    default:
      return [store, []]
  }
}
