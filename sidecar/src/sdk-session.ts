// sidecar/src/sdk-session.ts
// Create/resume/fork SDK sessions with long-lived stream loops (V1 query() API).
// All session state mutations go through SessionRegistry.emitSequenced().
import { EventEmitter } from 'node:events'
import {
  type Options,
  type PermissionMode,
  type SDKMessage,
  listSessions,
  query,
} from '@anthropic-ai/claude-agent-sdk'
import { findClaudeExecutable } from './cli-path.js'
import { mapSdkMessage } from './event-mapper.js'
import { MessageBridge } from './message-bridge.js'
import { updateModelCacheFromSession } from './model-cache.js'
import { PermissionHandler } from './permission-handler.js'
import type {
  AvailableSession,
  CreateSessionRequest,
  ForkSessionRequest,
  ResumeSessionRequest,
  ServerEvent,
} from './protocol.js'
import { RingBuffer } from './ring-buffer.js'
import type { ControlSession, SessionRegistry } from './session-registry.js'

function buildQueryOptions(
  opts: {
    model: string
    permissionMode?: string
    allowedTools?: string[]
    disallowedTools?: string[]
    projectPath?: string
    resume?: string
    forkSession?: boolean
  },
  permissions: PermissionHandler,
  emitFn: (event: ServerEvent) => void,
  abort: AbortController,
): Options {
  return {
    pathToClaudeCodeExecutable: findClaudeExecutable(),
    settingSources: ['user', 'project'],
    model: opts.model,
    ...(opts.permissionMode
      ? { permissionMode: opts.permissionMode as PermissionMode | undefined }
      : {}),
    ...(opts.allowedTools ? { allowedTools: opts.allowedTools } : {}),
    ...(opts.disallowedTools ? { disallowedTools: opts.disallowedTools } : {}),
    cwd: opts.projectPath || process.cwd(),
    ...(opts.resume ? { resume: opts.resume } : {}),
    ...(opts.forkSession ? { forkSession: true } : {}),
    includePartialMessages: true,
    abortController: abort,
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
  const bridge = new MessageBridge()
  const abort = new AbortController()

  // Pre-queue initial message if provided
  if (req.initialMessage) {
    bridge.push({
      type: 'user',
      session_id: '',
      message: { role: 'user', content: [{ type: 'text', text: req.initialMessage }] },
      parent_tool_use_id: null,
    })
  }

  // biome-ignore lint/style/useConst: definite assignment pattern — cs assigned below
  let cs!: ControlSession

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  const q = query({
    prompt: bridge,
    options: buildQueryOptions(
      {
        model: req.model,
        permissionMode: req.permissionMode,
        allowedTools: req.allowedTools,
        disallowedTools: req.disallowedTools,
        projectPath: req.projectPath,
      },
      permissions,
      emit,
      abort,
    ),
  })

  cs = {
    controlId,
    sessionId: '',
    model: req.model,
    query: q,
    bridge,
    abort,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    eventBuffer: new RingBuffer(200),
    nextSeq: 0,
    permissions,
    permissionMode: req.permissionMode ?? 'default',
    activeWs: null,
  }

  registry.register(cs)

  // Echo initial message into ring buffer (seq 0)
  if (req.initialMessage) {
    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: req.initialMessage,
      timestamp: Date.now() / 1000,
    })
  }

  runStreamLoop(cs, registry)

  return cs
}

export async function resumeControlSession(
  req: ResumeSessionRequest,
  registry: SessionRegistry,
): Promise<ControlSession> {
  const existing = registry.getBySessionId(req.sessionId)
  if (existing) {
    // If the requested mode differs from the existing session's mode,
    // close the old session and re-resume with the new mode.
    // This is needed for bypassPermissions which can't be set mid-session
    // via setPermissionMode() — it must be passed at query() init time.
    if (req.permissionMode && req.permissionMode !== existing.permissionMode) {
      closeSession(existing, registry)
      // Fall through to create new session with the requested mode
    } else {
      return existing
    }
  }

  const controlId = crypto.randomUUID()
  const emitter = new EventEmitter()
  const permissions = new PermissionHandler()
  const bridge = new MessageBridge()
  const abort = new AbortController()

  // biome-ignore lint/style/useConst: definite assignment pattern — cs assigned below
  let cs!: ControlSession

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  const q = query({
    prompt: bridge,
    options: buildQueryOptions(
      {
        model: req.model ?? 'claude-sonnet-4-20250514',
        permissionMode: req.permissionMode,
        projectPath: req.projectPath,
        resume: req.sessionId,
      },
      permissions,
      emit,
      abort,
    ),
  })

  cs = {
    controlId,
    sessionId: req.sessionId,
    model: req.model ?? 'claude-sonnet-4-20250514',
    query: q,
    bridge,
    abort,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    eventBuffer: new RingBuffer(200),
    permissionMode: req.permissionMode ?? 'default',
    nextSeq: 0,
    permissions,
    activeWs: null,
  }

  registry.register(cs)
  runStreamLoop(cs, registry)

  return cs
}

export function forkControlSession(
  req: ForkSessionRequest,
  registry: SessionRegistry,
): ControlSession {
  const controlId = crypto.randomUUID()
  const emitter = new EventEmitter()
  const permissions = new PermissionHandler()
  const bridge = new MessageBridge()
  const abort = new AbortController()

  // biome-ignore lint/style/useConst: definite assignment pattern — cs assigned below
  let cs!: ControlSession

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  const q = query({
    prompt: bridge,
    options: buildQueryOptions(
      {
        model: req.model ?? 'claude-sonnet-4-20250514',
        permissionMode: req.permissionMode,
        resume: req.sessionId,
        forkSession: true,
      },
      permissions,
      emit,
      abort,
    ),
  })

  cs = {
    controlId,
    sessionId: '', // will be populated from SDK message session_id
    model: req.model ?? 'claude-sonnet-4-20250514',
    query: q,
    bridge,
    abort,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    eventBuffer: new RingBuffer(200),
    nextSeq: 0,
    permissions,
    permissionMode: req.permissionMode ?? 'default',
    activeWs: null,
  }

  registry.register(cs)
  runStreamLoop(cs, registry)

  return cs
}

/** One long-lived stream loop per session. Runs until session closes or errors. */
function runStreamLoop(cs: ControlSession, registry: SessionRegistry): void {
  void (async () => {
    let modelCacheRefreshed = false
    try {
      for await (const msg of cs.query) {
        updateSessionStateFromRawMsg(cs, msg)
        const events = mapSdkMessage(msg)
        for (const event of events) {
          updateSessionState(cs, event)
          registry.emitSequenced(cs, event)

          // On first session_init, refresh model cache from SDK (fire-and-forget).
          // SDK is the source of truth for available models.
          if (event.type === 'session_init' && !modelCacheRefreshed) {
            modelCacheRefreshed = true
            cs.query
              .supportedModels()
              .then((models) => updateModelCacheFromSession(models))
              .catch((err) => console.warn('[model-cache] Failed to refresh from session:', err))
          }
        }
      }
      // Stream ended normally
      cs.state = 'closed'
    } catch (err) {
      // If closeReason is set, this was an expected close (user or shutdown)
      if (cs.closeReason) {
        cs.state = 'closed'
      } else {
        cs.state = 'error'
        registry.emitSequenced(cs, {
          type: 'error',
          message: err instanceof Error ? err.message : String(err),
          fatal: true,
        })
      }
    } finally {
      if (cs.state === 'closed') {
        registry.emitSequenced(cs, {
          type: 'session_closed',
          reason: cs.closeReason ?? 'stream_ended',
        })
      }
      bridge_close_safe(cs)
      setTimeout(() => registry.remove(cs.controlId), 5_000)
    }
  })()
}

/** Safely close the bridge, ignoring if already closed. */
function bridge_close_safe(cs: ControlSession): void {
  try {
    cs.bridge.close()
  } catch {
    // already closed — no-op
  }
}

/**
 * Extract session_id from raw SDK message and populate cs.sessionId.
 * Emits 'session_id_ready' event so waitForSessionInit can resolve.
 */
function updateSessionStateFromRawMsg(cs: ControlSession, msg: SDKMessage): void {
  const rawMsg = msg as SDKMessage & { session_id?: string }
  if (!cs.sessionId && rawMsg.session_id) {
    cs.sessionId = rawMsg.session_id
    cs.emitter.emit('session_id_ready', cs.sessionId)
  }
}

/** Update ControlSession state from emitted protocol events. */
function updateSessionState(cs: ControlSession, event: ServerEvent): void {
  switch (event.type) {
    case 'session_init':
      cs.state = 'waiting_input'
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
      cs.state = 'waiting_input'
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
  }
}

export function sendMessage(cs: ControlSession, content: string): void {
  cs.state = 'active'
  cs.bridge.push({
    type: 'user',
    session_id: cs.sessionId,
    message: { role: 'user', content: [{ type: 'text', text: content }] },
    parent_tool_use_id: null,
  })
}

export function closeSession(cs: ControlSession, registry: SessionRegistry): void {
  cs.permissions.drainAll()
  cs.closeReason = 'user_closed'
  cs.bridge.close()
  cs.query.return(undefined)
  // Do NOT emit session_closed here — runStreamLoop's finally block handles it
  // Do NOT remove from registry — runStreamLoop schedules delayed removal
}

/**
 * Change permission mode mid-session.
 * V1 Query has setPermissionMode() for streaming input mode.
 * Only allowed when NOT actively streaming (state !== 'active').
 */
export async function setSessionMode(
  cs: ControlSession,
  mode: string,
  registry: SessionRegistry,
): Promise<{ ok: boolean; currentMode: string }> {
  if (cs.state === 'active') {
    registry.emitSequenced(cs, {
      type: 'error',
      message:
        'Cannot change mode while agent is processing. Wait for the current turn to complete.',
      fatal: false,
    })
    return { ok: false, currentMode: cs.permissionMode }
  }

  await cs.query.setPermissionMode(mode as PermissionMode)
  cs.permissionMode = mode
  return { ok: true, currentMode: mode }
}

/**
 * Wait for session_id to be extracted from the first SDK message.
 *
 * Listens for 'session_id_ready' event (emitted by updateSessionStateFromRawMsg)
 * and fast-fails on fatal errors emitted on the 'message' channel.
 */
export function waitForSessionInit(cs: ControlSession, timeoutMs = 15_000): Promise<void> {
  if (cs.sessionId) return Promise.resolve()

  return new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => {
      cleanup()
      reject(new Error(`Session initialization timed out (${timeoutMs}ms)`))
    }, timeoutMs)

    const onSessionId = (_sessionId: string) => {
      cleanup()
      resolve()
    }

    const onMessage = (event: { type: string; message?: string; fatal?: boolean }) => {
      if (event.type === 'error' && event.fatal) {
        cleanup()
        reject(new Error(event.message ?? 'Session init failed'))
      }
    }

    const cleanup = () => {
      clearTimeout(timeout)
      cs.emitter.off('session_id_ready', onSessionId)
      cs.emitter.off('message', onMessage)
    }

    cs.emitter.on('session_id_ready', onSessionId)
    cs.emitter.on('message', onMessage)
  })
}

export async function listAvailableSessions(): Promise<AvailableSession[]> {
  const sessions = await listSessions()
  return sessions.map((s) => ({
    sessionId: s.sessionId,
    summary: s.summary,
    lastModified: s.lastModified,
    fileSize: s.fileSize ?? 0,
    customTitle: s.customTitle,
    firstPrompt: s.firstPrompt,
    gitBranch: s.gitBranch,
    cwd: s.cwd,
  }))
}
