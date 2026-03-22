import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { connTransition } from '../modules/conn-health'
import { metaTransition } from '../modules/meta'
import { outboxTransition } from '../modules/outbox'
import { turnTransition } from '../modules/turn'
import type { ChatPanelStore, Command, RawEvent, TransitionResult } from '../types'

/**
 * Merge server blocks (from accumulator) with existing blocks (from FETCH_HISTORY).
 * On resume, the sidecar accumulator starts fresh — it only has the current turn.
 * History blocks (from FETCH_HISTORY) are preserved by keeping any block whose ID
 * doesn't appear in the incoming set, then appending the incoming blocks.
 *
 * For new sessions: existing is empty → returns incoming unchanged.
 * For reconnects: accumulator has everything → history blocks are all in incoming → preserved is empty.
 */
function mergeBlocks(
  existing: ConversationBlock[],
  incoming: ConversationBlock[],
): ConversationBlock[] {
  if (existing.length === 0) return incoming
  const incomingIds = new Set(incoming.map((b) => b.id))
  const preserved = existing.filter((b) => !incomingIds.has(b.id))
  return [...preserved, ...incoming]
}

export function handleSdkOwned(store: ChatPanelStore, event: RawEvent): TransitionResult {
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
            blocks: mergeBlocks(p.blocks, event.blocks),
            turn: turnTransition(p.turn, { type: 'BLOCKS_UPDATE' }),
          },
        },
        [],
      ]

    case 'BLOCKS_SNAPSHOT':
      return [{ ...store, panel: { ...p, blocks: mergeBlocks(p.blocks, event.blocks) } }, []]

    // HISTORY_OK race: history API response arrives after we're already sdk_owned
    // (SELECT_SESSION fired FETCH_HISTORY + CHECK_SIDECAR_ACTIVE in parallel)
    // Only merge if we have no blocks yet (don't overwrite live blocks with stale history)
    case 'HISTORY_OK':
      if (p.blocks.length === 0) {
        return [{ ...store, panel: { ...p, blocks: event.blocks } }, []]
      }
      return [store, []]

    case 'TURN_COMPLETE': {
      const turn = turnTransition(p.turn, { type: 'TURN_COMPLETE' })
      const meta = metaTransition(store.meta, {
        type: 'TURN_USAGE',
        totalInputTokens: event.totalInputTokens,
        contextWindowSize: event.contextWindowSize,
      })
      return [
        {
          ...store,
          panel: { ...p, turn, blocks: mergeBlocks(p.blocks, event.blocks), pendingText: '' },
          meta,
        },
        [],
      ]
    }

    case 'TURN_ERROR': {
      const turn = turnTransition(p.turn, { type: 'TURN_ERROR' })
      const meta = metaTransition(store.meta, {
        type: 'TURN_USAGE',
        totalInputTokens: event.totalInputTokens,
        contextWindowSize: event.contextWindowSize,
      })
      return [
        {
          ...store,
          panel: { ...p, turn, blocks: mergeBlocks(p.blocks, event.blocks), pendingText: '' },
          meta,
        },
        [],
      ]
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
        [{ cmd: 'WS_SEND', message: { type: 'user_message', content: event.text } }],
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
