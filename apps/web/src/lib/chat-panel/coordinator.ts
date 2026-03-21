import { acquiringTransition } from './modules/acquiring'
import { connTransition } from './modules/conn-health'
import { metaTransition } from './modules/meta'
import { nobodyTransition } from './modules/nobody'
import { outboxTransition } from './modules/outbox'
import { turnTransition } from './modules/turn'
import type { ChatPanelStore, Command, PanelState, RawEvent, TransitionResult } from './types'

export function coordinate(store: ChatPanelStore, event: RawEvent): TransitionResult {
  // ── Global events (any phase) ────────────────────────────────
  switch (event.type) {
    case 'DESELECT':
      return [
        { panel: { phase: 'empty' }, outbox: { messages: [] }, meta: null },
        [{ cmd: 'CLOSE_SIDECAR_WS' }, { cmd: 'CLOSE_TERMINAL_WS' }],
      ]
    case 'SELECT_SESSION':
      return [
        {
          panel: { phase: 'nobody', sessionId: event.sessionId, sub: { sub: 'loading' } },
          outbox: { messages: [] },
          meta: null,
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

// ── empty ────────────────────────────────────────────────────────
function handleEmpty(store: ChatPanelStore, event: RawEvent): TransitionResult {
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

// ── nobody ───────────────────────────────────────────────────────
function handleNobody(store: ChatPanelStore, event: RawEvent): TransitionResult {
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

// ── cc_cli ───────────────────────────────────────────────────────
function handleCcCli(store: ChatPanelStore, event: RawEvent): TransitionResult {
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

// ── acquiring ────────────────────────────────────────────────────
function handleAcquiring(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'acquiring') return [store, []]

  // SSE race rejection: ignore live status during acquire
  if (event.type === 'LIVE_STATUS_CHANGED') return [store, []]

  // E-B2: Map WS events at coordinator level
  if (event.type === 'WS_OPEN' && p.step.step === 'ws_connecting') {
    const panel: PanelState = {
      ...p,
      step: { step: 'ws_initializing', controlId: p.step.controlId },
    }
    return [
      { ...store, panel },
      [
        {
          cmd: 'START_TIMER',
          id: 'init-timeout',
          delayMs: 10_000,
          event: { type: 'INIT_TIMEOUT' },
        },
      ],
    ]
  }

  if (event.type === 'WS_CLOSE' && p.step.step === 'ws_connecting') {
    // Non-recoverable WS close during connecting → action_failed
    return exitAcquiring(store, p, {
      stay: false,
      exit: 'action_failed',
      error: `WebSocket closed (code: ${event.code})`,
    })
  }

  // BLOCKS_SNAPSHOT during acquiring: update display blocks but stay
  if (event.type === 'BLOCKS_SNAPSHOT') {
    return [{ ...store, panel: { ...p, historyBlocks: event.blocks } }, []]
  }

  // Delegate to acquiring leaf
  if (
    event.type === 'ACQUIRE_OK' ||
    event.type === 'ACQUIRE_FAILED' ||
    event.type === 'SESSION_INIT' ||
    event.type === 'INIT_TIMEOUT'
  ) {
    const leafEvent = event.type === 'SESSION_INIT' ? { type: 'SESSION_INIT' as const } : event

    const result = acquiringTransition(p.step, leafEvent)
    if (result.stay) {
      let panel: PanelState = { ...p, step: result.state }
      const cmds: Command[] = []

      // ACQUIRE_OK → open WS + update sessionId if create/fork returned one
      if (event.type === 'ACQUIRE_OK' && result.state.step === 'ws_connecting') {
        cmds.push({ cmd: 'OPEN_SIDECAR_WS', sessionId: p.sessionId })
        if (event.sessionId) {
          panel = { ...panel, sessionId: event.sessionId, targetSessionId: event.sessionId }
        }
      }

      return [{ ...store, panel }, cmds]
    }

    return exitAcquiring(store, p, result)
  }

  return [store, []]
}

function exitAcquiring(
  store: ChatPanelStore,
  p: Extract<PanelState, { phase: 'acquiring' }>,
  result: { stay: false; exit: string; controlId?: string; sessionId?: string; error?: string },
): TransitionResult {
  if (result.exit === 'active') {
    const sessionId = result.sessionId ?? p.sessionId
    const controlId = result.controlId ?? ''
    const panel: PanelState = {
      phase: 'sdk_owned',
      sessionId,
      controlId,
      blocks: p.historyBlocks,
      pendingText: '',
      ephemeral: p.action === 'create',
      turn: { turn: 'idle' },
      conn: { health: 'ok' },
    }

    const cmds: Command[] = [{ cmd: 'CANCEL_TIMER', id: 'init-timeout' }]

    // Drain outbox: send all queued messages
    let outbox = store.outbox
    for (const msg of outbox.messages) {
      if (msg.status === 'queued') {
        cmds.push({ cmd: 'WS_SEND', message: { type: 'user_message', text: msg.text } })
        outbox = outboxTransition(outbox, { type: 'MARK_SENT', localId: msg.localId })
      }
    }

    return [{ panel, outbox, meta: store.meta }, cmds]
  }

  const error = result.error ?? 'Unknown error'

  if (result.exit === 'action_failed') {
    const panel: PanelState = {
      phase: 'recovering',
      sessionId: p.sessionId,
      blocks: p.historyBlocks,
      recovering: { kind: 'action_failed', error },
    }
    return [{ ...store, panel }, [{ cmd: 'TOAST', message: error, variant: 'error' }]]
  }

  if (result.exit === 'ws_fatal') {
    const panel: PanelState = {
      phase: 'recovering',
      sessionId: p.sessionId,
      blocks: p.historyBlocks,
      recovering: { kind: 'ws_fatal', error },
    }
    return [
      { ...store, panel },
      [{ cmd: 'CLOSE_SIDECAR_WS' }, { cmd: 'TOAST', message: error, variant: 'error' }],
    ]
  }

  return [store, []]
}

// ── sdk_owned ────────────────────────────────────────────────────
function handleSdkOwned(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'sdk_owned') return [store, []]

  switch (event.type) {
    // ── Turn events ──
    case 'STREAM_DELTA': {
      const turn = turnTransition(p.turn, { type: 'STREAM_DELTA' })
      return [{ ...store, panel: { ...p, turn, pendingText: p.pendingText + event.text } }, []]
    }

    case 'BLOCKS_UPDATE':
      return [
        {
          ...store,
          panel: {
            ...p,
            blocks: event.blocks,
            turn: turnTransition(p.turn, { type: 'BLOCKS_UPDATE' }),
          },
        },
        [],
      ]

    case 'BLOCKS_SNAPSHOT':
      return [{ ...store, panel: { ...p, blocks: event.blocks } }, []]

    case 'TURN_COMPLETE': {
      const turn = turnTransition(p.turn, { type: 'TURN_COMPLETE' })
      const meta = metaTransition(store.meta, {
        type: 'TURN_USAGE',
        totalInputTokens: event.totalInputTokens,
        contextWindowSize: event.contextWindowSize,
      })
      return [{ ...store, panel: { ...p, turn, blocks: event.blocks, pendingText: '' }, meta }, []]
    }

    case 'TURN_ERROR': {
      const turn = turnTransition(p.turn, { type: 'TURN_ERROR' })
      const meta = metaTransition(store.meta, {
        type: 'TURN_USAGE',
        totalInputTokens: event.totalInputTokens,
        contextWindowSize: event.contextWindowSize,
      })
      return [{ ...store, panel: { ...p, turn, blocks: event.blocks, pendingText: '' }, meta }, []]
    }

    case 'PERMISSION_REQUEST': {
      const turn = turnTransition(p.turn, {
        type: 'PERMISSION_REQUEST',
        kind: event.kind,
        requestId: event.requestId,
      })
      return [{ ...store, panel: { ...p, turn } }, []]
    }

    case 'SESSION_COMPACTING': {
      const turn = turnTransition(p.turn, { type: 'SESSION_COMPACTING' })
      return [{ ...store, panel: { ...p, turn } }, []]
    }

    case 'COMPACT_DONE': {
      const turn = turnTransition(p.turn, { type: 'COMPACT_DONE' })
      return [{ ...store, panel: { ...p, turn } }, []]
    }

    // ── User actions ──
    case 'SEND_MESSAGE': {
      const outbox = outboxTransition(store.outbox, {
        type: 'QUEUE',
        localId: event.localId,
        text: event.text,
      })
      const sentOutbox = outboxTransition(outbox, {
        type: 'MARK_SENT',
        localId: event.localId,
      })
      return [
        { ...store, outbox: sentOutbox },
        [{ cmd: 'WS_SEND', message: { type: 'user_message', text: event.text } }],
      ]
    }

    case 'INTERRUPT':
      return [store, [{ cmd: 'WS_SEND', message: { type: 'interrupt' } }]]

    case 'RESPOND_PERMISSION':
      return [
        store,
        [
          {
            cmd: 'WS_SEND',
            message: {
              type: 'permission_response',
              requestId: event.requestId,
              allowed: event.allowed,
              updatedPermissions: event.updatedPermissions,
            },
          },
        ],
      ]

    case 'ANSWER_QUESTION':
      return [
        store,
        [
          {
            cmd: 'WS_SEND',
            message: {
              type: 'question_response',
              requestId: event.requestId,
              answers: event.answers,
            },
          },
        ],
      ]

    case 'APPROVE_PLAN':
      return [
        store,
        [
          {
            cmd: 'WS_SEND',
            message: {
              type: 'plan_response',
              requestId: event.requestId,
              approved: event.approved,
              feedback: event.feedback,
            },
          },
        ],
      ]

    case 'SUBMIT_ELICITATION':
      return [
        store,
        [
          {
            cmd: 'WS_SEND',
            message: {
              type: 'elicitation_response',
              requestId: event.requestId,
              response: event.response,
            },
          },
        ],
      ]

    case 'SET_PERMISSION_MODE': {
      const meta = metaTransition(store.meta, {
        type: 'SERVER_MODE_CONFIRMED',
        mode: event.mode,
      })
      return [
        { ...store, meta },
        [{ cmd: 'WS_SEND', message: { type: 'set_mode', mode: event.mode } }],
      ]
    }

    // ── Conn health ──
    case 'WS_CLOSE': {
      const result = connTransition(p.conn, {
        type: 'WS_CLOSE',
        recoverable: event.recoverable,
      })
      if (result.stay) {
        const cmds: Command[] = []
        if (result.state.health === 'reconnecting') {
          cmds.push({
            cmd: 'START_TIMER',
            id: 'reconnect',
            delayMs: 1000 * result.state.attempt,
            event: { type: 'RECONNECT_ATTEMPT' },
          })
        }
        return [{ ...store, panel: { ...p, conn: result.state } }, cmds]
      }
      // Fatal WS close
      if (!result.stay && result.exit === 'ws_fatal') {
        const wsError = result.error ?? 'WebSocket fatal'
        return [
          {
            ...store,
            panel: {
              phase: 'recovering',
              sessionId: p.sessionId,
              blocks: p.blocks,
              recovering: { kind: 'ws_fatal', error: wsError },
            },
          },
          [{ cmd: 'CLOSE_SIDECAR_WS' }, { cmd: 'TOAST', message: wsError, variant: 'error' }],
        ]
      }
      return [store, []]
    }

    case 'WS_OPEN': {
      const result = connTransition(p.conn, { type: 'WS_OPEN' })
      if (result.stay) {
        return [
          { ...store, panel: { ...p, conn: result.state } },
          [{ cmd: 'CANCEL_TIMER', id: 'reconnect' }],
        ]
      }
      return [store, []]
    }

    case 'RECONNECT_ATTEMPT': {
      const result = connTransition(p.conn, { type: 'RECONNECT_ATTEMPT' })
      if (result.stay) {
        const cmds: Command[] = []
        if (result.state.health === 'reconnecting') {
          cmds.push({
            cmd: 'OPEN_SIDECAR_WS',
            sessionId: p.sessionId,
          })
        }
        return [{ ...store, panel: { ...p, conn: result.state } }, cmds]
      }
      // Max retries exceeded → recovering
      if (!result.stay && result.exit === 'ws_fatal') {
        const wsError = result.error ?? 'WebSocket fatal'
        return [
          {
            ...store,
            panel: {
              phase: 'recovering',
              sessionId: p.sessionId,
              blocks: p.blocks,
              recovering: { kind: 'ws_fatal', error: wsError },
            },
          },
          [{ cmd: 'CLOSE_SIDECAR_WS' }, { cmd: 'TOAST', message: wsError, variant: 'error' }],
        ]
      }
      return [store, []]
    }

    // ── Session lifecycle ──
    case 'SESSION_CLOSED':
      return [
        {
          ...store,
          panel: {
            phase: 'closed',
            sessionId: p.sessionId,
            blocks: p.blocks,
            ephemeral: p.ephemeral,
          },
        },
        [{ cmd: 'CLOSE_SIDECAR_WS' }],
      ]

    // ── Meta events ──
    case 'SESSION_INIT': {
      const meta = metaTransition(store.meta, event)
      return [{ ...store, meta }, []]
    }

    case 'SERVER_MODE_CONFIRMED': {
      const meta = metaTransition(store.meta, event)
      return [{ ...store, meta }, []]
    }

    case 'SERVER_MODE_REJECTED':
      return [
        store,
        [
          {
            cmd: 'TOAST',
            message: event.reason ?? `Mode ${event.mode} rejected`,
            variant: 'error',
          },
        ],
      ]

    case 'COMMANDS_UPDATED': {
      const meta = metaTransition(store.meta, event)
      return [{ ...store, meta }, []]
    }

    case 'AGENTS_UPDATED': {
      const meta = metaTransition(store.meta, event)
      return [{ ...store, meta }, []]
    }

    default:
      return [store, []]
  }
}

// ── recovering ───────────────────────────────────────────────────
function handleRecovering(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'recovering') return [store, []]

  switch (event.type) {
    case 'SEND_MESSAGE': {
      const outbox = outboxTransition(store.outbox, {
        type: 'QUEUE',
        localId: event.localId,
        text: event.text,
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
      return [{ panel, outbox, meta: store.meta }, [{ cmd: 'POST_RESUME', sessionId: p.sessionId }]]
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

// ── closed ───────────────────────────────────────────────────────
function handleClosed(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'closed') return [store, []]

  switch (event.type) {
    case 'SEND_MESSAGE': {
      const outbox = outboxTransition(store.outbox, {
        type: 'QUEUE',
        localId: event.localId,
        text: event.text,
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
      return [{ panel, outbox, meta: store.meta }, [{ cmd: 'POST_RESUME', sessionId: p.sessionId }]]
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
