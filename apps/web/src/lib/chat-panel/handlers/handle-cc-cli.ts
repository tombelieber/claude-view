import type {
  ChatPanelStore,
  HistoryPagination,
  PanelState,
  RawEvent,
  TransitionResult,
} from '../types'

const PAGE_SIZE = 100

export function handleCcCli(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'cc_cli') return [store, []]

  switch (event.type) {
    case 'TAKEOVER_CLI': {
      return [
        { ...store, panel: { ...p, sub: { sub: 'takeover_killing' } } },
        [{ cmd: 'KILL_CLI_SESSION', sessionId: p.sessionId }],
      ]
    }

    case 'KILL_CLI_OK': {
      const panel: PanelState = {
        phase: 'acquiring',
        sessionId: p.sessionId,
        targetSessionId: null,
        action: 'resume',
        historyBlocks: p.blocks,
        pendingMessage: null,
        step: { step: 'posting' },
      }
      return [
        { ...store, panel },
        [
          {
            cmd: 'POST_RESUME',
            sessionId: p.sessionId,
            projectPath: store.projectPath ?? undefined,
          },
        ],
      ]
    }

    case 'KILL_CLI_FAILED':
      return [
        { ...store, panel: { ...p, sub: { sub: 'watching' } } },
        [{ cmd: 'TOAST', message: event.error, variant: 'error' }],
      ]

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

    case 'LOAD_OLDER_HISTORY': {
      const pg = store.historyPagination
      if (!pg || pg.offset <= 0 || pg.fetchingOlder) return [store, []]
      const newOffset = Math.max(0, pg.offset - PAGE_SIZE)
      const limit = pg.offset - newOffset
      return [
        { ...store, historyPagination: { ...pg, fetchingOlder: true } },
        [{ cmd: 'FETCH_OLDER_HISTORY', sessionId: p.sessionId, offset: newOffset, limit }],
      ]
    }

    case 'OLDER_HISTORY_OK': {
      const blocks = [...event.blocks, ...p.blocks]
      return [
        {
          ...store,
          panel: { ...p, blocks },
          historyPagination: store.historyPagination
            ? { ...store.historyPagination, offset: event.offset, fetchingOlder: false }
            : null,
        },
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
