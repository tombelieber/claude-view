// sidecar/src/ws-handler.ts
import type { PermissionUpdate } from '@anthropic-ai/claude-agent-sdk'
import type { WebSocket } from 'ws'
import type { ClientMessage, ResumeMsg } from './protocol.js'
import { sendMessage, setSessionMode } from './sdk-session.js'
import type { SessionRegistry } from './session-registry.js'

export function handleWebSocket(ws: WebSocket, controlId: string, registry: SessionRegistry) {
  const session = registry.get(controlId)
  if (!session) {
    ws.send(JSON.stringify({ type: 'error', message: 'Session not found', fatal: true }))
    ws.close()
    return
  }

  // Enforce one WS per session: close old connection before subscribing new one
  if (session.activeWs && session.activeWs.readyState === ws.OPEN) {
    session.activeWs.close(4001, 'replaced_by_new_connection')
  }
  session.activeWs = ws

  // Subscribe to session events
  const onMessage = (msg: unknown) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(msg))
    }
  }
  session.emitter.on('message', onMessage)

  // Send current state
  registry.emitSequenced(session, {
    type: 'session_status',
    status: session.state === 'compacting' ? 'compacting' : null,
  })

  // Heartbeat config (no seq)
  ws.send(JSON.stringify({ type: 'heartbeat_config', intervalMs: 15_000 }))

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

        case 'permission_response':
          session.permissions.resolvePermission(
            msg.requestId,
            msg.allowed,
            msg.updatedPermissions as PermissionUpdate[] | undefined,
          )
          break

        case 'question_response':
          if (!session.permissions.resolveQuestion(msg.requestId, msg.answers)) {
            ws.send(
              JSON.stringify({
                type: 'error',
                message: 'Unknown question requestId',
                fatal: false,
              }),
            )
          }
          break

        case 'plan_response':
          if (!session.permissions.resolvePlan(msg.requestId, msg.approved, msg.feedback)) {
            ws.send(
              JSON.stringify({ type: 'error', message: 'Unknown plan requestId', fatal: false }),
            )
          }
          break

        case 'elicitation_response':
          if (!session.permissions.resolveElicitation(msg.requestId, msg.response)) {
            ws.send(
              JSON.stringify({
                type: 'error',
                message: 'Unknown elicitation requestId',
                fatal: false,
              }),
            )
          }
          break

        case 'resume': {
          const lastSeq = (msg as ResumeMsg).lastSeq
          const missed = session.eventBuffer.getAfter(lastSeq, (e) => e.seq)
          if (missed === null) {
            ws.send(
              JSON.stringify({ type: 'error', message: 'replay_buffer_exhausted', fatal: true }),
            )
            ws.close()
          } else {
            for (const event of missed) {
              if (ws.readyState === ws.OPEN) ws.send(JSON.stringify(event.msg))
            }
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
                data: cmds,
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
                data: agents,
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

  // Cleanup on close — drain interactive maps, keep session alive for reconnect
  ws.on('close', () => {
    if (session.activeWs === ws) {
      session.activeWs = null
    }
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
