import { handleAcquiring } from './handlers/handle-acquiring'
import { handleCcCli } from './handlers/handle-cc-cli'
import { handleClosed } from './handlers/handle-closed'
import { handleEmpty } from './handlers/handle-empty'
import { handleNobody } from './handlers/handle-nobody'
import { handleRecovering } from './handlers/handle-recovering'
import { handleSdkOwned } from './handlers/handle-sdk-owned'
import type { ChatPanelStore, RawEvent, TransitionResult } from './types'

export function coordinate(store: ChatPanelStore, event: RawEvent): TransitionResult {
  // ── Global events (any phase) ────────────────────────────────
  switch (event.type) {
    case 'DESELECT':
      return [
        { panel: { phase: 'empty' }, outbox: { messages: [] }, meta: null, projectPath: null },
        [{ cmd: 'CLOSE_SIDECAR_WS' }, { cmd: 'CLOSE_TERMINAL_WS' }],
      ]
    case 'SELECT_SESSION':
      return [
        {
          panel: { phase: 'nobody', sessionId: event.sessionId, sub: { sub: 'loading' } },
          outbox: { messages: [] },
          meta: null,
          projectPath: event.projectPath ?? null,
        },
        [
          { cmd: 'FETCH_HISTORY', sessionId: event.sessionId },
          { cmd: 'CHECK_SIDECAR_ACTIVE', sessionId: event.sessionId },
        ],
      ]
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
