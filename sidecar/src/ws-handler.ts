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
        case 'question_response': {
          const ok = sessions.resolveQuestion(controlId, msg.requestId, msg.answers)
          if (!ok)
            ws.send(
              JSON.stringify({
                type: 'error',
                message: 'Unknown question requestId',
                fatal: false,
              }),
            )
          break
        }
        case 'plan_response': {
          const ok = sessions.resolvePlan(controlId, msg.requestId, msg.approved, msg.feedback)
          if (!ok)
            ws.send(
              JSON.stringify({ type: 'error', message: 'Unknown plan requestId', fatal: false }),
            )
          break
        }
        case 'elicitation_response': {
          const ok = sessions.resolveElicitation(controlId, msg.requestId, msg.response)
          if (!ok)
            ws.send(
              JSON.stringify({
                type: 'error',
                message: 'Unknown elicitation requestId',
                fatal: false,
              }),
            )
          break
        }
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

    // Drain interactive pending maps (question/plan/elicitation) — these have
    // no auto-timeout timer, so they'd hang forever without a connected frontend.
    // Do NOT call sessions.close() — that destroys the SDK session and defeats
    // the frontend's reconnect logic (exponential backoff, up to 10 retries).
    // pendingPermissions is NOT drained here because it has its own 60s auto-deny timer.
    for (const [, pending] of session.pendingQuestions) {
      pending.resolve({}) // empty answers → allow with no selections
    }
    session.pendingQuestions.clear()

    for (const [, pending] of session.pendingPlans) {
      pending.resolve({ approved: false }) // reject plan (never auto-approve)
    }
    session.pendingPlans.clear()

    for (const [, pending] of session.pendingElicitations) {
      pending.resolve('') // empty response
    }
    session.pendingElicitations.clear()
  })
}
