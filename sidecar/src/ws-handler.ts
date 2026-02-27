// sidecar/src/ws-handler.ts
import type { WebSocket } from 'ws'
import type { SessionManager } from './session-manager.js'
import type { ClientMessage, ServerMessage } from './types.js'

export function handleWebSocket(ws: WebSocket, controlId: string, sessions: SessionManager) {
  const session = sessions.getSession(controlId)
  if (!session) {
    ws.send(JSON.stringify({ type: 'error', message: 'Session not found', fatal: true }))
    ws.close()
    return
  }

  // Subscribe to session events via EventEmitter
  const onMessage = (msg: ServerMessage) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(msg))
    }
  }
  session.emitter.on('message', onMessage)

  // Send initial status
  ws.send(
    JSON.stringify({
      type: 'session_status',
      status: session.status,
      contextUsage: session.contextUsage,
      turnCount: session.turnCount,
    } satisfies ServerMessage),
  )

  // Handle incoming messages from frontend
  ws.on('message', (raw) => {
    try {
      const msg: ClientMessage = JSON.parse(raw.toString())
      switch (msg.type) {
        case 'user_message':
          sessions.sendMessage(controlId, msg.content).catch((err) => {
            ws.send(JSON.stringify({ type: 'error', message: String(err), fatal: false }))
          })
          break
        case 'permission_response':
          sessions.resolvePermission(controlId, msg.requestId, msg.allowed)
          break
        case 'ping':
          ws.send(JSON.stringify({ type: 'pong' }))
          break
      }
    } catch {
      ws.send(JSON.stringify({ type: 'error', message: 'Invalid message format', fatal: false }))
    }
  })

  // Cleanup on close
  ws.on('close', () => {
    session.emitter.removeListener('message', onMessage)
  })
}
