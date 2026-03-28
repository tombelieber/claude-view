// sidecar/src/sdk-session.ts
// Create/resume/fork SDK sessions with long-lived stream loops (V1 query() API).
// All session state mutations go through SessionRegistry.emitSequenced().
import { EventEmitter } from 'node:events'
import { existsSync, readdirSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import {
  type Options,
  type PermissionMode,
  type SDKMessage,
  getSessionInfo,
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
import type { ControlSession, SessionRegistry } from './session-registry.js'
import { StreamAccumulator } from './stream-accumulator.js'

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
    // Always allow bypassPermissions so setPermissionMode() can switch to it mid-session.
    // Per SDK docs, setPermissionMode("bypassPermissions") is a valid dynamic change.
    allowDangerouslySkipPermissions: true,
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

// ─── Generic session setup ──────────────────────────────────────
// All session types (create/resume/fork) share the same wiring.
// Differences are expressed via SessionSetupOpts — no ad-hoc per-type logic.

interface SessionSetupOpts {
  /** Model to use. */
  model: string
  /** Permission mode ('default', 'plan', 'bypassPermissions', etc.). */
  permissionMode?: string
  /** Allowed/disallowed tools — create-only, ignored for resume/fork. */
  allowedTools?: string[]
  disallowedTools?: string[]
  /** Resolved project path (cwd for the SDK). */
  projectPath?: string
  /** Session ID to resume from — set for resume and fork, absent for create. */
  resume?: string
  /** Branch conversation history into a new session. Requires resume. */
  forkSession?: boolean
  /** Known session ID — set for resume (known up front), empty for create/fork (SDK assigns). */
  knownSessionId?: string
  /** Initial user message to queue before the stream loop starts. */
  initialMessage?: string
}

function setupControlSession(opts: SessionSetupOpts, registry: SessionRegistry): ControlSession {
  const controlId = crypto.randomUUID()
  const emitter = new EventEmitter()
  const permissions = new PermissionHandler()
  const bridge = new MessageBridge()
  const abort = new AbortController()

  // Pre-queue initial message so SDK gets it before session_init.
  if (opts.initialMessage) {
    bridge.push({
      type: 'user',
      session_id: '',
      message: { role: 'user', content: [{ type: 'text', text: opts.initialMessage }] },
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
        model: opts.model,
        permissionMode: opts.permissionMode,
        allowedTools: opts.allowedTools,
        disallowedTools: opts.disallowedTools,
        projectPath: opts.projectPath,
        resume: opts.resume,
        forkSession: opts.forkSession,
      },
      permissions,
      emit,
      abort,
    ),
  })

  cs = {
    controlId,
    sessionId: opts.knownSessionId ?? '',
    model: opts.model,
    query: q,
    bridge,
    abort,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    permissions,
    permissionMode: opts.permissionMode ?? 'default',
    wsClients: new Set(),
    lastSessionInit: null,
    accumulator: new StreamAccumulator(),
  }

  registry.register(cs)

  // Echo initial message into accumulator so blocks_snapshot includes it.
  if (opts.initialMessage) {
    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: opts.initialMessage,
      timestamp: Date.now() / 1000,
    })
  }

  runStreamLoop(cs, registry)

  return cs
}

/**
 * Shared pre-flight for resume and fork: verify session file exists on disk,
 * resolve projectPath from session metadata if not provided.
 *
 * IMPORTANT: Uses filesystem existence check, NOT getSessionInfo().
 * getSessionInfo() filters out sessions with "no extractable summary" (e.g. sessions
 * interrupted before any assistant response) and returns undefined — but the JSONL file
 * exists on disk and the SDK can resume it. Using getSessionInfo as an existence check
 * was a recurring bug (fixed 7+ times as a "projectPath issue" when it was really this).
 */
async function resolveExistingSession(
  sessionId: string,
  projectPath?: string,
): Promise<string | undefined> {
  if (!sessionJsonlExists(sessionId)) {
    throw new Error(
      `Session ${sessionId} not found in CLI session store. It may have been deleted or belongs to a different project path.`,
    )
  }
  let resolved = projectPath
  try {
    const info = await getSessionInfo(sessionId, {
      dir: projectPath || undefined,
    })
    if (info?.cwd && !resolved) {
      resolved = info.cwd
    }
  } catch {
    // getSessionInfo failures are non-fatal — only used for cwd fallback
  }
  return resolved
}

// ─── Public API ─────────────────────────────────────────────────

export function createControlSession(
  req: CreateSessionRequest,
  registry: SessionRegistry,
): ControlSession {
  return setupControlSession(
    {
      model: req.model,
      permissionMode: req.permissionMode,
      allowedTools: req.allowedTools,
      disallowedTools: req.disallowedTools,
      projectPath: req.projectPath,
      initialMessage: req.initialMessage,
    },
    registry,
  )
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

  const resolvedPath = await resolveExistingSession(req.sessionId, req.projectPath)

  return setupControlSession(
    {
      model: req.model ?? 'claude-sonnet-4-20250514',
      permissionMode: req.permissionMode,
      projectPath: resolvedPath,
      resume: req.sessionId,
      knownSessionId: req.sessionId,
      initialMessage: req.initialMessage,
    },
    registry,
  )
}

export async function forkControlSession(
  req: ForkSessionRequest,
  registry: SessionRegistry,
): Promise<ControlSession> {
  const resolvedPath = await resolveExistingSession(req.sessionId, req.projectPath)

  return setupControlSession(
    {
      model: req.model ?? 'claude-sonnet-4-20250514',
      permissionMode: req.permissionMode,
      projectPath: resolvedPath,
      resume: req.sessionId,
      forkSession: true,
      initialMessage: req.initialMessage,
    },
    registry,
  )
}

/** One long-lived stream loop per session. Runs until session closes or errors. */
function runStreamLoop(cs: ControlSession, registry: SessionRegistry): void {
  void (async () => {
    let modelCacheRefreshed = false
    try {
      for await (const msg of cs.query) {
        updateSessionStateFromRawMsg(cs, msg)
        const events = mapSdkMessage(msg)
        const raw = msg as unknown as Record<string, unknown>
        for (const event of events) {
          updateSessionState(cs, event)
          registry.emitSequenced(cs, event, raw)

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
        console.warn(
          `[stream-loop] ${cs.sessionId.slice(0, 8)} error: ${err instanceof Error ? err.message : String(err)}`,
        )
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
      registry.scheduleRemove(cs.controlId, 5_000)
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
 * Wait for the SDK session to fully initialize (session_init event emitted).
 *
 * For new sessions: waits for session_id extraction + session_init.
 * For resumed sessions: sessionId is already known, but still waits for session_init
 * so that lastSessionInit is cached before the HTTP response triggers WS connections.
 */
export function waitForSessionInit(cs: ControlSession, timeoutMs = 15_000): Promise<void> {
  // Only skip if BOTH sessionId is set AND session_init has been received
  if (cs.sessionId && cs.lastSessionInit) return Promise.resolve()

  return new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => {
      cleanup()
      reject(new Error(`Session initialization timed out (${timeoutMs}ms)`))
    }, timeoutMs)

    const onSessionId = (_sessionId: string) => {
      // For resumed sessions, sessionId arrives immediately but we still need session_init
      if (cs.lastSessionInit) {
        cleanup()
        resolve()
      }
    }

    const onMessage = (event: { type: string; message?: string; fatal?: boolean }) => {
      if (event.type === 'session_init') {
        // session_init received — lastSessionInit is now cached by emitSequenced
        if (cs.sessionId) {
          cleanup()
          resolve()
        }
      }
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

/**
 * Filesystem existence check for a session JSONL file.
 * Searches all project directories under ~/.claude/projects/.
 * This is intentionally separate from getSessionInfo() which filters out
 * sessions with no extractable summary (e.g. interrupted before assistant response).
 */
export function sessionJsonlExists(sessionId: string): boolean {
  const projectsDir = join(homedir(), '.claude', 'projects')
  try {
    for (const dir of readdirSync(projectsDir)) {
      if (existsSync(join(projectsDir, dir, `${sessionId}.jsonl`))) {
        return true
      }
    }
  } catch {
    // If ~/.claude/projects doesn't exist, session can't exist
  }
  return false
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
