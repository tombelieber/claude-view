// sidecar/src/sdk-session.ts
// Create/resume SDK sessions with long-lived stream loops.
// All session state mutations go through SessionRegistry.emitSequenced().
import { EventEmitter } from 'node:events'
import {
  type SDKSessionOptions,
  listSessions,
  unstable_v2_createSession,
  unstable_v2_resumeSession,
} from '@anthropic-ai/claude-agent-sdk'
import { mapSdkMessage } from './event-mapper.js'
import { PermissionHandler } from './permission-handler.js'
import type {
  AvailableSession,
  CreateSessionRequest,
  ResumeSessionRequest,
  ServerEvent,
} from './protocol.js'
import { RingBuffer } from './ring-buffer.js'
import type { ControlSession, SessionRegistry } from './session-registry.js'

function buildSdkOptions(
  opts: {
    model: string
    permissionMode?: string
    allowedTools?: string[]
    disallowedTools?: string[]
    projectPath?: string
  },
  permissions: PermissionHandler,
  emitFn: (event: ServerEvent) => void,
): SDKSessionOptions {
  return {
    model: opts.model,
    ...(opts.permissionMode
      ? { permissionMode: opts.permissionMode as SDKSessionOptions['permissionMode'] }
      : {}),
    ...(opts.allowedTools ? { allowedTools: opts.allowedTools } : {}),
    ...(opts.disallowedTools ? { disallowedTools: opts.disallowedTools } : {}),
    ...(opts.projectPath ? { cwd: opts.projectPath } : {}),
    // canUseTool: SDK V0.2.72 passes (toolName, input, options) where options
    // includes { signal, suggestions?, blockedPath?, decisionReason?, toolUseID, agentID? }.
    canUseTool: async (toolName, input, toolOpts) => {
      return permissions.handleCanUseTool(toolName, input, toolOpts, emitFn)
    },
  }
}

export function createControlSession(
  req: CreateSessionRequest,
  registry: SessionRegistry,
): ControlSession {
  const controlId = crypto.randomUUID()
  const emitter = new EventEmitter()
  const permissions = new PermissionHandler()

  // cs must be created before SDK session so the closure in buildSdkOptions can
  // reference registry.emitSequenced(cs, ...) after cs is assigned.
  // We use definite assignment (let cs!: ControlSession) then assign atomically.
  // biome-ignore lint/style/useConst: definite assignment pattern — cs assigned below
  let cs!: ControlSession

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  const sdkSession = unstable_v2_createSession(
    buildSdkOptions(
      {
        model: req.model,
        permissionMode: req.permissionMode,
        allowedTools: req.allowedTools,
        disallowedTools: req.disallowedTools,
        projectPath: req.projectPath,
      },
      permissions,
      emit,
    ),
  )

  cs = {
    controlId,
    sessionId: '', // filled after stream emits session_init with session_id
    model: req.model,
    sdkSession,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    eventBuffer: new RingBuffer(200),
    nextSeq: 0,
    permissions,
  }

  registry.register(cs)

  // Start long-lived stream loop
  runStreamLoop(cs, registry)

  return cs
}

export async function resumeControlSession(
  req: ResumeSessionRequest,
  registry: SessionRegistry,
): Promise<ControlSession> {
  // Return existing session if already active
  const existing = registry.getBySessionId(req.sessionId)
  if (existing) return existing

  const controlId = crypto.randomUUID()
  const emitter = new EventEmitter()
  const permissions = new PermissionHandler()

  // biome-ignore lint/style/useConst: definite assignment pattern — cs assigned below
  let cs!: ControlSession

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  const sdkSession = unstable_v2_resumeSession(
    req.sessionId,
    buildSdkOptions(
      {
        model: req.model ?? 'claude-sonnet-4-20250514',
        permissionMode: req.permissionMode,
        projectPath: req.projectPath,
      },
      permissions,
      emit,
    ),
  )

  cs = {
    controlId,
    sessionId: req.sessionId,
    model: req.model ?? 'claude-sonnet-4-20250514',
    sdkSession,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    eventBuffer: new RingBuffer(200),
    nextSeq: 0,
    permissions,
  }

  registry.register(cs)

  // Start long-lived stream loop
  runStreamLoop(cs, registry)

  return cs
}

/** One long-lived stream loop per session. Runs until session closes or errors. */
function runStreamLoop(cs: ControlSession, registry: SessionRegistry): void {
  void (async () => {
    try {
      for await (const msg of cs.sdkSession.stream()) {
        const events = mapSdkMessage(msg)
        for (const event of events) {
          updateSessionState(cs, event)
          registry.emitSequenced(cs, event)
        }
      }
      // Stream ended normally
      cs.state = 'closed'
      registry.emitSequenced(cs, { type: 'session_closed', reason: 'stream_ended' })
      // Brief delay so any reconnecting WS can replay the ring buffer before removal
      setTimeout(() => registry.remove(cs.controlId), 5_000)
    } catch (err) {
      cs.state = 'error'
      registry.emitSequenced(cs, {
        type: 'error',
        message: err instanceof Error ? err.message : String(err),
        fatal: true,
      })
      setTimeout(() => registry.remove(cs.controlId), 5_000)
    }
  })()
}

/** Update ControlSession state from emitted protocol events (before forwarding to clients). */
function updateSessionState(cs: ControlSession, event: ServerEvent): void {
  switch (event.type) {
    case 'session_init':
      cs.state = 'waiting_input'
      // Capture sessionId from SDK now that it's initialized (create flow)
      if (!cs.sessionId) {
        try {
          cs.sessionId = cs.sdkSession.sessionId
        } catch {
          // sessionId not yet available — will be set on next event
        }
      }
      break
    case 'assistant_text':
    case 'assistant_thinking':
    case 'tool_use_start':
      cs.state = 'active'
      break
    case 'turn_complete':
      cs.state = 'waiting_input'
      cs.totalCostUsd = event.totalCostUsd
      cs.turnCount = event.numTurns
      cs.modelUsage = event.modelUsage as Record<string, unknown>
      break
    case 'turn_error':
      cs.state = 'waiting_input' // allow retry
      cs.totalCostUsd = event.totalCostUsd
      cs.turnCount = event.numTurns
      break
    case 'permission_request':
    case 'ask_question':
    case 'plan_approval':
    case 'elicitation':
      cs.state = 'waiting_permission'
      break
    case 'session_status':
      if (event.status === 'compacting') cs.state = 'compacting'
      else if (cs.state === 'compacting') cs.state = 'waiting_input'
      break
    case 'session_closed':
      cs.state = 'closed'
      break
    // All other events do not change session state
  }
}

export async function sendMessage(cs: ControlSession, content: string): Promise<void> {
  cs.state = 'active'
  await cs.sdkSession.send(content)
}

export async function closeSession(cs: ControlSession, registry: SessionRegistry): Promise<void> {
  cs.permissions.drainAll()
  cs.sdkSession.close()
  registry.remove(cs.controlId)
}

/**
 * Change permission mode mid-session.
 * V2 SDK has no setPermissionMode() — must close and re-resume with new mode.
 * Only allowed when NOT actively streaming (state !== 'active').
 */
export function setSessionMode(cs: ControlSession, mode: string, registry: SessionRegistry): void {
  if (cs.state === 'active') {
    registry.emitSequenced(cs, {
      type: 'error',
      message:
        'Cannot change mode while agent is processing. Wait for the current turn to complete.',
      fatal: false,
    })
    return
  }

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  cs.sdkSession.close()
  cs.sdkSession = unstable_v2_resumeSession(
    cs.sessionId,
    buildSdkOptions({ model: cs.model, permissionMode: mode }, cs.permissions, emit),
  )

  // Restart stream loop for the new SDK session handle
  runStreamLoop(cs, registry)
}

/**
 * Wait for session_init event to populate cs.sessionId.
 *
 * New sessions start with sessionId='' because the SDK hasn't initialized yet.
 * The stream loop emits session_init once the SDK is ready, at which point
 * updateSessionState sets cs.sessionId = cs.sdkSession.sessionId.
 *
 * Exported for testing.
 */
export function waitForSessionInit(cs: ControlSession, timeoutMs = 15_000): Promise<void> {
  if (cs.sessionId) return Promise.resolve()

  return new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => {
      cs.emitter.off('message', handler)
      reject(new Error(`Session initialization timed out (${timeoutMs}ms)`))
    }, timeoutMs)

    const handler = (event: { type: string; message?: string; fatal?: boolean }) => {
      if (event.type === 'session_init') {
        // Don't resolve if sessionId is still empty — updateSessionState's try/catch
        // may have silently failed to read sdkSession.sessionId. Keep waiting for the
        // next event that populates it, or timeout.
        if (!cs.sessionId) return
        clearTimeout(timeout)
        cs.emitter.off('message', handler)
        resolve()
      } else if (event.type === 'error' && event.fatal) {
        clearTimeout(timeout)
        cs.emitter.off('message', handler)
        reject(new Error(event.message ?? 'Session init failed'))
      }
    }
    cs.emitter.on('message', handler)
  })
}

export async function listAvailableSessions(): Promise<AvailableSession[]> {
  const sessions = await listSessions()
  return sessions.map((s) => ({
    sessionId: s.sessionId,
    summary: s.summary,
    lastModified: s.lastModified,
    fileSize: s.fileSize,
    customTitle: s.customTitle,
    firstPrompt: s.firstPrompt,
    gitBranch: s.gitBranch,
    cwd: s.cwd,
  }))
}
