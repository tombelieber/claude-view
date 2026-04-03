import type {
  ChatPanelStore,
  HistoryPagination,
  PanelState,
  RawEvent,
  TransitionResult,
} from '../types'

export function handleCcCli(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'cc_cli') return [store, []]

  switch (event.type) {
    case 'TAKEOVER_CLI': {
      // Fork instead of kill+resume: creates a new session from the conversation
      // history without killing the CLI process. Eliminates the race condition where
      // the CLI doesn't release the session lock in time (15s init timeout).
      const panel: PanelState = {
        phase: 'acquiring',
        sessionId: p.sessionId,
        targetSessionId: null,
        action: 'fork',
        historyBlocks: p.blocks,
        pendingMessage: null,
        step: { step: 'posting' },
      }
      return [
        { ...store, panel },
        [
          { cmd: 'CLOSE_TERMINAL_WS' },
          {
            cmd: 'POST_FORK',
            sessionId: p.sessionId,
            projectPath: store.projectPath ?? undefined,
          },
        ],
      ]
    }

    case 'LIVE_STATUS_CHANGED': {
      const updatedStore = event.projectPath ? { ...store, projectPath: event.projectPath } : store
      if (event.status === 'inactive') {
        const panel: PanelState = {
          phase: 'nobody',
          sessionId: p.sessionId,
          sub: { sub: 'ready', blocks: p.blocks },
        }
        return [{ ...updatedStore, panel }, [{ cmd: 'CLOSE_TERMINAL_WS' }]]
      }
      return [updatedStore, []]
    }

    case 'HISTORY_OK': {
      // History may arrive after cc_cli transition (race: FETCH_HISTORY vs LIVE_STATUS)
      let pagination: HistoryPagination | null = store.historyPagination
      if (event.total != null && event.offset != null) {
        pagination = { total: event.total, offset: event.offset, fetchingOlder: false }
      }
      return [
        { ...store, panel: { ...p, blocks: event.blocks }, historyPagination: pagination },
        [],
      ]
    }

    case 'TERMINAL_BLOCK': {
      // Live block from terminal WS block stream — merge by ID.
      // Same ID → replace (block updated), new ID → append.
      const existing = p.blocks.findIndex((b) => b.id === event.block.id)
      const blocks =
        existing >= 0
          ? p.blocks.map((b, i) => (i === existing ? event.block : b))
          : [...p.blocks, event.block]
      return [{ ...store, panel: { ...p, blocks } }, []]
    }

    case 'TERMINAL_CONNECTED':
      // Terminal WS finished scrollback — no-op, just marks readiness
      return [store, []]

    default:
      return [store, []]
  }
}
