import { handleAcquiring } from './handlers/handle-acquiring'
import { handleCcCli } from './handlers/handle-cc-cli'
import { handleClosed } from './handlers/handle-closed'
import { handleEmpty } from './handlers/handle-empty'
import { handleNobody } from './handlers/handle-nobody'
import { handleRecovering } from './handlers/handle-recovering'
import { handleSdkOwned } from './handlers/handle-sdk-owned'
import { handleLoadOlder, handleOlderHistoryOk } from './modules/pagination'
import type { ChatPanelStore, RawEvent, TransitionResult } from './types'

export function coordinate(store: ChatPanelStore, event: RawEvent): TransitionResult {
  // ── Global events (any phase) ────────────────────────────────
  switch (event.type) {
    case 'DESELECT':
      return [
        {
          panel: { phase: 'empty' },
          outbox: { messages: [] },
          meta: null,
          projectPath: null,
          lastModel: null,
          lastPermissionMode: null,
          historyPagination: null,
        },
        [{ cmd: 'CLOSE_SIDECAR_WS' }, { cmd: 'CLOSE_TERMINAL_WS' }],
      ]
    case 'SELECT_SESSION':
      return [
        {
          panel: { phase: 'nobody', sessionId: event.sessionId, sub: { sub: 'loading' } },
          outbox: { messages: [] },
          meta: null,
          projectPath: event.projectPath ?? null,
          lastModel: null,
          lastPermissionMode: null,
          historyPagination: null,
        },
        [
          { cmd: 'FETCH_HISTORY', sessionId: event.sessionId },
          // FETCH_HOOK_EVENTS intentionally NOT fired here — ?format=block
          // already merges DB hook events server-side with correct pagination
          // positioning. Firing it here would double-fetch the same events
          // with different ID prefixes (hook-db- vs hook-), causing duplicates.
          // FETCH_HOOK_EVENTS is only needed after TURN_COMPLETE/TURN_ERROR
          // for live sessions where hook events are in memory (not yet in DB).
          { cmd: 'CHECK_SIDECAR_ACTIVE', sessionId: event.sessionId },
        ],
      ]
    case 'LOAD_OLDER_HISTORY':
      return handleLoadOlder(store)

    case 'OLDER_HISTORY_OK':
      return handleOlderHistoryOk(store, event)
  }

  // ── Phase-based routing ──────────────────────────────────────
  switch (store.panel.phase) {
    case 'empty':
      return handleEmpty(store, event)
    case 'nobody':
      return handleNobody(store, event)
    case 'cc_cli':
      return handleCcCli(store, event)
    case 'acquiring':
      return handleAcquiring(store, event)
    case 'sdk_owned':
      return handleSdkOwned(store, event)
    case 'recovering':
      return handleRecovering(store, event)
    case 'closed':
      return handleClosed(store, event)
  }
}
