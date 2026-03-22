import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { acquiringTransition } from '../modules/acquiring'
import { type MetaEvent, metaTransition } from '../modules/meta'
import { outboxTransition } from '../modules/outbox'
import type { ChatPanelStore, Command, PanelState, RawEvent, TransitionResult } from '../types'

/** Merge incoming blocks with existing, preserving non-overlapping history. */
function mergeBlocks(
  existing: ConversationBlock[],
  incoming: ConversationBlock[],
): ConversationBlock[] {
  if (existing.length === 0) return incoming
  const incomingIds = new Set(incoming.map((b) => b.id))
  const preserved = existing.filter((b) => !incomingIds.has(b.id))
  return [...preserved, ...incoming]
}

export function handleAcquiring(store: ChatPanelStore, event: RawEvent): TransitionResult {
  const p = store.panel
  if (p.phase !== 'acquiring') return [store, []]

  // SSE race rejection: ignore live status PHASE changes during acquire,
  // but still update projectPath if provided (it's safe — no phase change).
  if (event.type === 'LIVE_STATUS_CHANGED') {
    if (event.projectPath && event.projectPath !== store.projectPath) {
      return [{ ...store, projectPath: event.projectPath }, []]
    }
    return [store, []]
  }

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

  if (
    event.type === 'WS_CLOSE' &&
    (p.step.step === 'ws_connecting' || p.step.step === 'ws_initializing')
  ) {
    // WS close during connecting or initializing → action_failed immediately
    // (don't wait for init-timeout if WS is already dead)
    return exitAcquiring(store, p, {
      stay: false,
      exit: 'action_failed',
      error: `WebSocket closed (code: ${event.code})`,
    })
  }

  // BLOCKS_SNAPSHOT during acquiring: merge with history (don't replace — the
  // accumulator starts fresh on resume and would wipe history from FETCH_HISTORY).
  if (event.type === 'BLOCKS_SNAPSHOT') {
    return [
      { ...store, panel: { ...p, historyBlocks: mergeBlocks(p.historyBlocks, event.blocks) } },
      [],
    ]
  }

  // HISTORY_OK race: history API responds after we already entered acquiring
  // (SIDECAR_HAS_SESSION arrived before HISTORY_OK in nobody phase).
  // Merge with existing — snapshot may have already set some blocks.
  if (event.type === 'HISTORY_OK') {
    return [
      { ...store, panel: { ...p, historyBlocks: mergeBlocks(event.blocks, p.historyBlocks) } },
      [],
    ]
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

      // ACQUIRE_OK → open WS + update sessionId if create/fork returned one.
      // Use event.sessionId for the WS connection — p.sessionId is '' for creates.
      if (event.type === 'ACQUIRE_OK' && result.state.step === 'ws_connecting') {
        if (event.sessionId) {
          panel = { ...panel, sessionId: event.sessionId, targetSessionId: event.sessionId }
        }
        cmds.push({ cmd: 'OPEN_SIDECAR_WS', sessionId: event.sessionId ?? p.sessionId })
      }

      return [{ ...store, panel }, cmds]
    }

    // SESSION_INIT triggers exit to sdk_owned — populate meta with the full event
    // (the leaf only needs { type: 'SESSION_INIT' }, but meta needs model/capabilities/etc.)
    const storeForExit =
      event.type === 'SESSION_INIT'
        ? { ...store, meta: metaTransition(store.meta, event as MetaEvent) }
        : store
    return exitAcquiring(storeForExit, p, result)
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

    const cmds: Command[] = [
      { cmd: 'CANCEL_TIMER', id: 'init-timeout' },
      { cmd: 'INVALIDATE_SIDEBAR' },
    ]

    // Drain outbox: send all queued messages
    let outbox = store.outbox
    for (const msg of outbox.messages) {
      if (msg.status === 'queued') {
        cmds.push({ cmd: 'WS_SEND', message: { type: 'user_message', content: msg.text } })
        outbox = outboxTransition(outbox, { type: 'MARK_SENT', localId: msg.localId })
      }
    }

    return [
      {
        panel,
        outbox,
        meta: store.meta,
        projectPath: store.projectPath,
        lastModel: store.lastModel,
        lastPermissionMode: store.lastPermissionMode,
      },
      cmds,
    ]
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
