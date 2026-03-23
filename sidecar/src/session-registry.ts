// sidecar/src/session-registry.ts
import type { EventEmitter } from 'node:events'
import type { Query } from '@anthropic-ai/claude-agent-sdk'
import type { WebSocket } from 'ws'
import type { MessageBridge } from './message-bridge.js'
import type { PermissionHandler } from './permission-handler.js'
import type { ActiveSession, ServerEvent, SessionInit } from './protocol.js'
import type { StreamAccumulator } from './stream-accumulator.js'

export type SessionState =
  | 'initializing'
  | 'waiting_input'
  | 'active'
  | 'waiting_permission'
  | 'compacting'
  | 'error'
  | 'closed'

export interface ControlSession {
  controlId: string
  sessionId: string
  model: string
  query: Query
  bridge: MessageBridge
  abort: AbortController
  closeReason?: string
  state: SessionState
  totalCostUsd: number
  turnCount: number
  modelUsage: Record<string, unknown>
  startedAt: number
  emitter: EventEmitter
  permissions: PermissionHandler
  permissionMode: string
  wsClients: Set<WebSocket>
  lastSessionInit: SessionInit | null
  accumulator: StreamAccumulator
}

export class SessionRegistry {
  private sessions = new Map<string, ControlSession>()

  get(controlId: string): ControlSession | undefined {
    return this.sessions.get(controlId)
  }

  getBySessionId(sessionId: string): ControlSession | undefined {
    for (const cs of this.sessions.values()) {
      if (cs.sessionId === sessionId) return cs
    }
    return undefined
  }

  hasSessionId(sessionId: string): boolean {
    return this.getBySessionId(sessionId) !== undefined
  }

  register(cs: ControlSession): void {
    this.sessions.set(cs.controlId, cs)
  }

  remove(controlId: string): void {
    this.sessions.delete(controlId)
  }

  list(): ActiveSession[] {
    return Array.from(this.sessions.values()).map((cs) => ({
      controlId: cs.controlId,
      sessionId: cs.sessionId,
      state: cs.state,
      turnCount: cs.turnCount,
      totalCostUsd: cs.totalCostUsd || null,
      startedAt: cs.startedAt,
    }))
  }

  get activeCount(): number {
    return this.sessions.size
  }

  emitSequenced(
    cs: ControlSession,
    event: ServerEvent,
    rawSdkMessage?: Record<string, unknown>,
  ): void {
    // Cache session_init for late-joining WS clients
    if (event.type === 'session_init') {
      cs.lastSessionInit = event as SessionInit
    }
    // Push to accumulator BEFORE emitting so that blocks_update (sent by
    // ws-handler inside the 'message' listener) reflects the latest state.
    // Without this, blocks_update is one event stale — e.g. after assistant_text
    // the blocks_update would lack the text, causing pendingText duplication.
    //
    // Filter text-carrying deltas (prevents doubled text with assistant_text).
    // Keep structural events (content_block_start/stop) so accumulator builds
    // block skeletons that blocks_snapshot can deliver.
    const isTextDelta =
      event.type === 'stream_delta' &&
      (event as { deltaType?: string }).deltaType === 'content_block_delta'
    if (!isTextDelta) cs.accumulator.push(event, rawSdkMessage)
    cs.emitter.emit('message', event)
  }

  async closeAll(): Promise<void> {
    for (const cs of this.sessions.values()) {
      cs.closeReason = 'shutdown'
      cs.permissions.drainAll()
      cs.bridge.close()
      cs.query.close()
    }
    this.sessions.clear()
  }
}
