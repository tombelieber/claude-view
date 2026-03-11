// sidecar/src/ws-handler.ts
import type { PermissionUpdate } from '@anthropic-ai/claude-agent-sdk'
import type { WebSocket } from 'ws'
import type { ClientMessage, ResumeMsg, SetModeMsg } from './protocol.js'
import { sendMessage, setSessionMode } from './sdk-session.js'
import type { SessionRegistry } from './session-registry.js'

export function handleWebSocket(ws: WebSocket, controlId: string, registry: SessionRegistry) {
  const session = registry.get(controlId)
  if (!session) {
    ws.send(JSON.stringify({ type: 'error', message: 'Session not found', fatal: true }))
    ws.close()
    return
  }

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
  ws.on('message', (raw) => {
    try {
      const msg: ClientMessage = JSON.parse(raw.toString())
      switch (msg.type) {
        case 'user_message':
          sendMessage(session, msg.content).catch((err) => {
            ws.send(JSON.stringify({ type: 'error', message: String(err), fatal: false }))
          })
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

        case 'set_mode': {
          const modeMsg = msg as SetModeMsg
          const VALID_MODES = new Set([
            'default',
            'acceptEdits',
            'bypassPermissions',
            'plan',
            'dontAsk',
          ])
          if (!VALID_MODES.has(modeMsg.mode)) {
            ws.send(
              JSON.stringify({
                type: 'error',
                message: `Invalid mode: ${modeMsg.mode}`,
                fatal: false,
              }),
            )
            break
          }
          // V2 SDK: close + re-resume with new permission mode (synchronous — errors emitted via registry)
          try {
            setSessionMode(session, modeMsg.mode, registry)
          } catch (err: unknown) {
            ws.send(
              JSON.stringify({
                type: 'error',
                message: `Mode change failed: ${err}`,
                fatal: false,
              }),
            )
          }
          break
        }
      }
    } catch {
      ws.send(JSON.stringify({ type: 'error', message: 'Invalid message format', fatal: false }))
    }
  })

  // Cleanup on close — drain interactive maps, keep session alive for reconnect
  ws.on('close', () => {
    session.emitter.removeListener('message', onMessage)
    session.permissions.drainInteractive()
  })
}
