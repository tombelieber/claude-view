import type { ChatPanelStore, PanelState, RawEvent, TransitionResult } from '../types'

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
        historyBlocks: [],
        pendingMessage: null,
        step: { step: 'posting' },
      }
      return [{ ...store, panel }, [{ cmd: 'POST_RESUME', sessionId: p.sessionId }]]
    }

    case 'KILL_CLI_FAILED':
      return [
        { ...store, panel: { ...p, sub: { sub: 'watching' } } },
        [{ cmd: 'TOAST', message: event.error, variant: 'error' }],
      ]

    case 'LIVE_STATUS_CHANGED': {
      if (event.status === 'inactive') {
        const panel: PanelState = {
          phase: 'nobody',
          sessionId: p.sessionId,
          sub: { sub: 'ready', blocks: [] },
        }
        return [{ ...store, panel }, [{ cmd: 'CLOSE_TERMINAL_WS' }]]
      }
      return [store, []]
    }

    default:
      return [store, []]
  }
}
