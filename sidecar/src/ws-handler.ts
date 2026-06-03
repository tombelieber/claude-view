// sidecar/src/ws-handler.ts
import type { WebSocket } from 'ws'
import { normalizeNameArray } from './event-mapper.js'
import { resolveInteraction } from './interaction-resolver.js'
import type { ClientMessage, ServerEvent } from './protocol.js'
import { sendMessage, setSessionMode } from './sdk-session.js'
import type { SessionRegistry } from './session-registry.js'

// Events that modify blocks in the accumulator and need immediate blocks_update to the client.
// Without this, the client won't see new blocks until the next unrelated update.
//
// RULE: if StreamAccumulator.ingest() calls pushInteraction/pushNotice/pushSystem/
// handleUserMessage or modifies currentAssistant for an event type, it belongs here.
// Exceptions: turn_complete/turn_error are handled separately (line ~58) with full blocks.
const BLOCK_PRODUCING_EVENTS = new Set([
  // Streaming content — builds/extends assistant blocks
  'assistant_text',
  'assistant_thinking',
  'tool_use_start',
  'tool_use_result',
  'user_message_echo',
  // Interactive — creates InteractionBlock (permission card, question card, etc.)
  'permission_request',
  'ask_question',
  'plan_approval',
  'elicitation',
  // Notices — rate limit, errors, compaction
  'rate_limit',
  'context_compacted',
  'assistant_error',
  // Tool lifecycle — updates existing assistant block segments
  'tool_progress',
  'tool_summary',
  // System blocks — informational events that pushSystem() into the accumulator
  'elicitation_complete',
  'hook_event',
  'task_started',
  'task_progress',
  'task_notification',
  'files_saved',
  'command_output',
  'session_status',
  'unknown_sdk_event',
  'prompt_suggestion',
  'auth_status',
  // Error notice
  'error',
  // NOTE: turn_complete/turn_error send blocks directly (handled separately in onMessage).
  // session_init is followed by blocks_snapshot. session_closed ends the session.
  // content_block_start is handled by the isBlockStart check below.
  // stream_delta (non-text) is handled by isBlockStart. pong produces no block.
])

// WS backpressure thresholds. A live conversation snapshot (getBlocks()) can be
// large, and a block-producing event fires one on nearly every streaming tick.
// If a client (backgrounded tab, slow link) stops draining, queuing a fresh full
// snapshot per event grows the ws send buffer without bound — a sidecar OOM
// vector for long sessions. So we (a) skip the redundant full snapshot while the
// buffer is over SOFT (the next under-limit event or turn_complete carries the
// latest full state — getBlocks() is always current and the client merges by id),
// and (b) close a client that blows past HARD (it is effectively dead; retaining
// its queue would leak memory). Small per-event messages still flow until HARD.
const WS_SNAPSHOT_SOFT_LIMIT_BYTES = 4 * 1024 * 1024
const WS_CLOSE_HARD_LIMIT_BYTES = 32 * 1024 * 1024

/** Bytes currently queued in the socket's send buffer. Mocks and exotic
 *  transports may not expose `bufferedAmount`; treat absent as 0 so the happy
 *  path (and unit-test mocks) behave exactly as before. */
function bufferedBytes(ws: WebSocket): number {
  return (ws as { bufferedAmount?: number }).bufferedAmount ?? 0
}

/** Send if the socket is keeping up; drop a client that has fallen too far
 *  behind instead of letting its backlog OOM the sidecar. */
function trySend(ws: WebSocket, payload: string): void {
  if (ws.readyState !== ws.OPEN) return
  if (bufferedBytes(ws) > WS_CLOSE_HARD_LIMIT_BYTES) {
    // 1013 = "Try Again Later".
    ws.close(1013, 'client too slow to keep up')
    return
  }
  ws.send(payload)
}

export function handleWebSocket(ws: WebSocket, controlId: string, registry: SessionRegistry) {
  const session = registry.get(controlId)
  if (!session) {
    ws.send(JSON.stringify({ type: 'error', message: 'Session not found', fatal: true }))
    ws.close()
    return
  }

  // Add this WS to the multi-client set
  session.wsClients.add(ws)

  // 1. Re-send cached session_init if available (V2 multi-tab behavior)
  if (session.lastSessionInit) {
    ws.send(JSON.stringify(session.lastSessionInit))
  }

  // 2. Send session_status
  ws.send(
    JSON.stringify({
      type: 'session_status',
      status: session.state === 'compacting' ? 'compacting' : null,
    }),
  )

  // 3. Heartbeat config
  ws.send(JSON.stringify({ type: 'heartbeat_config', intervalMs: 15_000 }))

  // 4. Blocks snapshot
  ws.send(
    JSON.stringify({
      type: 'blocks_snapshot',
      blocks: session.accumulator.getBlocks(),
    }),
  )

  // 5. Subscribe to live events AFTER cached messages are sent.
  // If the listener were registered before step 1, the SDK's active stream
  // could relay events (blocks_update) that arrive on the client before
  // session_init — causing the frontend to process them in the wrong FSM phase.
  const onMessage = (rawMsg: unknown) => {
    if (ws.readyState !== ws.OPEN) return
    const msg = rawMsg as ServerEvent
    if (msg.type === 'turn_complete' || msg.type === 'turn_error') {
      // Terminal turn state — always carries the authoritative full blocks.
      trySend(ws, JSON.stringify({ ...msg, blocks: session.accumulator.getBlocks() }))
    } else {
      trySend(ws, JSON.stringify(msg))
      // Send blocks_update when accumulator state changes structurally:
      // - CONTENT_EVENTS: assistant_text, tool_use_start, assistant_thinking, user_message_echo
      // - content_block_start: creates the assistant block skeleton (needed for pendingText)
      const isBlockStart =
        msg.type === 'stream_delta' &&
        (msg as { deltaType?: string }).deltaType === 'content_block_start'
      if (BLOCK_PRODUCING_EVENTS.has(msg.type) || isBlockStart) {
        // Skip the (potentially large) full snapshot while the client is behind;
        // a later under-limit event or the turn_complete above carries the latest
        // state. The frontend merges blocks_update by id, so dropping intermediate
        // snapshots converges to the same result (see mergeBlocks).
        if (bufferedBytes(ws) <= WS_SNAPSHOT_SOFT_LIMIT_BYTES) {
          trySend(
            ws,
            JSON.stringify({
              type: 'blocks_update',
              blocks: session.accumulator.getBlocks(),
            }),
          )
        }
      }
    }
  }
  session.emitter.on('message', onMessage)

  // Handle incoming messages
  ws.on('message', async (raw) => {
    try {
      const msg: ClientMessage = JSON.parse(raw.toString())
      switch (msg.type) {
        case 'user_message':
          registry.emitSequenced(session, {
            type: 'user_message_echo',
            content: msg.content,
            timestamp: Date.now() / 1000,
          })
          sendMessage(session, msg.content)
          break

        // All four interaction decisions share one resolution primitive
        // (interaction-resolver.ts), so the WS path and the REST delivery
        // bridge can never drift. An unknown requestId returns {ok:false}.
        case 'permission_response':
        case 'question_response':
        case 'plan_response':
        case 'elicitation_response': {
          const result = resolveInteraction(session, msg)
          if (!result.ok) {
            ws.send(
              JSON.stringify({
                type: 'error',
                message: result.reason ?? 'Unknown requestId',
                fatal: false,
              }),
            )
          }
          break
        }

        case 'ping':
          ws.send(JSON.stringify({ type: 'pong' }))
          break

        case 'set_mode':
          try {
            const result = await setSessionMode(session, msg.mode, registry)
            if (result.ok) {
              ws.send(JSON.stringify({ type: 'mode_changed', mode: result.currentMode }))
            } else {
              ws.send(
                JSON.stringify({
                  type: 'mode_rejected',
                  mode: result.currentMode,
                  requestedMode: msg.mode,
                }),
              )
            }
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err)
            ws.send(
              JSON.stringify({
                type: 'mode_rejected',
                mode: session.permissionMode,
                requestedMode: msg.mode,
                error: message,
              }),
            )
          }
          break

        case 'interrupt':
          try {
            await session.query.interrupt()
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'set_model':
          try {
            await session.query.setModel(msg.model)
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'set_max_thinking_tokens':
          try {
            await session.query.setMaxThinkingTokens(msg.maxThinkingTokens)
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'stop_task':
          try {
            await session.query.stopTask(msg.taskId)
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'query_models':
          try {
            const models = await session.query.supportedModels()
            ws.send(
              JSON.stringify({
                type: 'query_result',
                queryType: 'models',
                data: models,
                requestId: msg.requestId,
              }),
            )
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'query_commands':
          try {
            const cmds = await session.query.supportedCommands()
            ws.send(
              JSON.stringify({
                type: 'query_result',
                queryType: 'commands',
                data: normalizeNameArray(cmds),
                requestId: msg.requestId,
              }),
            )
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'query_agents':
          try {
            const agents = await session.query.supportedAgents()
            ws.send(
              JSON.stringify({
                type: 'query_result',
                queryType: 'agents',
                data: normalizeNameArray(agents),
                requestId: msg.requestId,
              }),
            )
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'query_mcp_status':
          try {
            const status = await session.query.mcpServerStatus()
            ws.send(
              JSON.stringify({
                type: 'query_result',
                queryType: 'mcp_status',
                data: status,
                requestId: msg.requestId,
              }),
            )
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'query_account_info':
          try {
            const info = await session.query.accountInfo()
            ws.send(
              JSON.stringify({
                type: 'query_result',
                queryType: 'account_info',
                data: info,
                requestId: msg.requestId,
              }),
            )
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'reconnect_mcp':
          try {
            await session.query.reconnectMcpServer(msg.serverName)
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'toggle_mcp':
          try {
            await session.query.toggleMcpServer(msg.serverName, msg.enabled)
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'set_mcp_servers':
          try {
            // biome-ignore lint/suspicious/noExplicitAny: SDK type mismatch
            const result = await session.query.setMcpServers(msg.servers as any)
            ws.send(JSON.stringify({ type: 'mcp_set_result', result, requestId: msg.requestId }))
          } catch (err) {
            sendError(ws, err)
          }
          break

        case 'rewind_files':
          try {
            const result = await session.query.rewindFiles(msg.userMessageId, {
              dryRun: msg.dryRun,
            })
            ws.send(JSON.stringify({ type: 'rewind_result', result, requestId: msg.requestId }))
          } catch (err) {
            sendError(ws, err)
          }
          break
      }
    } catch {
      ws.send(JSON.stringify({ type: 'error', message: 'Invalid message format', fatal: false }))
    }
  })

  // Cleanup on close — remove from set, drain interactive maps
  ws.on('close', () => {
    session.wsClients.delete(ws)
    session.emitter.removeListener('message', onMessage)
    session.permissions.drainInteractive()
  })
}

function sendError(ws: WebSocket, err: unknown): void {
  const message = err instanceof Error ? err.message : String(err)
  if (ws.readyState === ws.OPEN) {
    ws.send(JSON.stringify({ type: 'error', message, fatal: false }))
  }
}
