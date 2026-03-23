import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { ActiveSession } from '@claude-view/shared/types/sidecar-protocol'
import type { ModelUsageInfo } from '@claude-view/shared/types/sidecar-protocol'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { clearRespondedCache } from '@claude-view/shared/components/conversation/blocks/shared/use-interaction-handlers'
import type { SessionState } from '../lib/derive-panel-mode'
import { SessionChannel } from '../lib/session-channel'
import { sidecarWsUrl } from '../lib/ws-url'
import { NON_RECOVERABLE_CODES } from '../types/control'

const INITIAL_BACKOFF_MS = 1000
const MAX_BACKOFF_MS = 30_000
const MAX_RECONNECT_ATTEMPTS = 10

/** Internal state: committed blocks from sidecar + pending streaming text */
export interface MessageState {
  committed: ConversationBlock[]
  pendingText: string
}

export interface SessionSourceResult {
  /** Committed blocks from sidecar snapshots/updates (backward compat alias for committedBlocks) */
  blocks: ConversationBlock[]
  /** Committed blocks from sidecar snapshots/updates */
  committedBlocks: ConversationBlock[]
  /** Pending streaming text not yet committed to a block */
  pendingText: string
  sessionState: SessionState
  controlId: string | null
  /** Send that may trigger session resume (for user_message only). */
  send: ((msg: Record<string, unknown>) => void) | null
  /** Send that only works when WS is open — for control commands (set_mode, interrupt, etc.).
   *  Never triggers session resume. Null when not connected. */
  sendIfLive: ((msg: Record<string, unknown>) => void) | null
  isLive: boolean
  reconnect: () => void
  resume: (permissionMode?: string, model?: string) => Promise<void>
  totalInputTokens: number
  contextWindowSize: number
  model: string
  slashCommands: string[]
  mcpServers: { name: string; status: string }[]
  permissionMode: string
  skills: string[]
  agents: string[]
  channel: SessionChannel | null
  capabilities: string[]
  clearPendingMessage: (text: string) => void
  /** True after init() has resolved (active-session check complete). Gates history fetching. */
  initComplete: boolean
}

/** Exported for testing — determines which send function to use based on connection state. */
export function deriveEffectiveSend(
  isLive: boolean,
  controlId: string | null,
  sessionId: string | undefined,
  send: ((msg: Record<string, unknown>) => void) | null,
  connectAndSend: (msg: Record<string, unknown>) => void,
): ((msg: Record<string, unknown>) => void) | null {
  if (isLive) return send // WS is open, use direct send
  if (controlId || sessionId) return connectAndSend // Lazy resumable or auto-resume
  return null // No session at all
}

export function useSessionSource(sessionId: string | undefined): SessionSourceResult {
  const [msgState, setMsgState] = useState<MessageState>({ committed: [], pendingText: '' })
  const [sessionState, setSessionState] = useState<SessionState>('idle')
  const [controlId, setControlId] = useState<string | null>(null)
  const [isLive, setIsLive] = useState(false)
  const [initComplete, setInitComplete] = useState(false)
  const [totalInputTokens, setTotalInputTokens] = useState(0)
  const [contextWindowSize, setContextWindowSize] = useState(0)
  const [model, setModel] = useState('')
  const [slashCommands, setSlashCommands] = useState<string[]>([])
  const [mcpServers, setMcpServers] = useState<{ name: string; status: string }[]>([])
  const [permissionMode, setPermissionMode] = useState('default')
  const [skills, setSkills] = useState<string[]>([])
  const [agents, setAgents] = useState<string[]>([])
  const [capabilities, setCapabilities] = useState<string[]>([])
  const channelRef = useRef(new SessionChannel(null))

  const wsRef = useRef<WebSocket | null>(null)
  const lastSeqRef = useRef(-1)
  const unmountedRef = useRef(false)
  // Ref mirror of controlId state — accessible in cleanup closures without stale captures.
  const controlIdRef = useRef<string | null>(null)
  const heartbeatTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const pongReceivedRef = useRef(true)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pendingMessagesRef = useRef<Record<string, unknown>[]>([])
  const resumingRef = useRef(false)

  // --- Heartbeat ---
  const clearHeartbeat = useCallback(() => {
    if (heartbeatTimerRef.current) {
      clearInterval(heartbeatTimerRef.current)
      heartbeatTimerRef.current = null
    }
  }, [])

  const startHeartbeat = useCallback(
    (intervalMs: number) => {
      clearHeartbeat()
      pongReceivedRef.current = true
      heartbeatTimerRef.current = setInterval(() => {
        if (!pongReceivedRef.current) {
          const ws = wsRef.current
          if (ws && ws.readyState === WebSocket.OPEN) {
            ws.close(4200, 'heartbeat_timeout')
          }
          clearHeartbeat()
          return
        }
        pongReceivedRef.current = false
        const ws = wsRef.current
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'ping' }))
        }
      }, intervalMs)
    },
    [clearHeartbeat],
  )

  // --- WS message handler ---
  const handleWsMessage = useCallback(
    (ws: WebSocket, event: MessageEvent) => {
      if (wsRef.current !== ws) return

      let raw: Record<string, unknown>
      try {
        raw = JSON.parse(event.data)
      } catch {
        return
      }

      if (raw.type === 'heartbeat_config') {
        startHeartbeat(raw.intervalMs as number)
        return
      }
      if (raw.type === 'pong') {
        pongReceivedRef.current = true
        return
      }

      const seq = (raw.seq as number) ?? -1
      if (seq > lastSeqRef.current) {
        lastSeqRef.current = seq
      }

      // Track session state from events
      switch (raw.type) {
        case 'session_init': {
          setSessionState('waiting_input')
          const init = raw as unknown as {
            model?: string
            slashCommands?: string[]
            mcpServers?: { name: string; status: string }[]
            permissionMode?: string
            skills?: string[]
            agents?: string[]
            capabilities?: string[]
          }
          if (init.model) setModel(init.model)
          if (init.slashCommands) setSlashCommands(init.slashCommands)
          if (init.mcpServers) setMcpServers(init.mcpServers)
          if (init.permissionMode) setPermissionMode(init.permissionMode)
          if (init.skills) setSkills(init.skills)
          if (init.agents) setAgents(init.agents)
          setCapabilities(init.capabilities ?? [])
          break
        }
        case 'session_status':
          if (raw.status === 'compacting') {
            setSessionState('compacting')
          }
          break
        case 'blocks_snapshot': {
          // Full block snapshot from sidecar — replaces all committed blocks
          const blocks = raw.blocks as ConversationBlock[]
          setMsgState({ committed: blocks, pendingText: '' })
          if (typeof raw.lastSeq === 'number') {
            lastSeqRef.current = raw.lastSeq as number
          }
          break
        }
        case 'blocks_update': {
          // Incremental block update — replaces committed blocks, clears pending
          const blocks = raw.blocks as ConversationBlock[]
          setMsgState({ committed: blocks, pendingText: '' })
          break
        }
        case 'turn_complete':
        case 'turn_error': {
          setSessionState('waiting_input')
          // If blocks payload is present, update committed blocks atomically
          if (raw.blocks) {
            const blocks = raw.blocks as ConversationBlock[]
            setMsgState({ committed: blocks, pendingText: '' })
          }
          // Keep existing cost/usage updates
          const mu = raw.modelUsage as Record<string, ModelUsageInfo> | undefined
          if (mu) {
            let sumInput = 0
            let ctxWindow = 0
            for (const info of Object.values(mu)) {
              sumInput +=
                (info.inputTokens ?? 0) +
                (info.cacheReadInputTokens ?? 0) +
                (info.cacheCreationInputTokens ?? 0)
              if (ctxWindow === 0 && info.contextWindow > 0) ctxWindow = info.contextWindow
            }
            setTotalInputTokens(sumInput)
            if (ctxWindow > 0) setContextWindowSize(ctxWindow)
          }
          break
        }
        case 'assistant_text':
        case 'tool_use_start':
          setSessionState('active')
          break
        case 'stream_delta': {
          setSessionState('active')
          // Append textDelta to pending text
          if (raw.textDelta) {
            const delta = raw.textDelta as string
            setMsgState((prev) => ({ ...prev, pendingText: prev.pendingText + delta }))
          }
          break
        }
        case 'permission_request':
        case 'ask_question':
        case 'plan_approval':
        case 'elicitation':
          setSessionState('waiting_permission')
          break
        case 'session_closed':
          setSessionState('closed')
          setIsLive(false)
          // Clear stale controlId so next send goes through auto-resume path.
          // Reset lastSeq so the next WS connect replays from the beginning.
          setControlId(null)
          lastSeqRef.current = -1
          break
        case 'mode_changed':
          setPermissionMode(raw.mode as string)
          break
        case 'mode_rejected':
          // Revert to the sidecar's actual mode
          setPermissionMode(raw.mode as string)
          toast.error('Mode change rejected', {
            description: (raw.reason as string) ?? (raw.requestedMode as string) ?? undefined,
          })
          break
        case 'query_result': {
          const evt = raw as { requestId?: string; queryType?: string; data: unknown }
          if (evt.requestId) channelRef.current.handleResponse(evt.requestId, evt.data)
          // Also update local state for commands/agents so palette auto-refreshes
          if (evt.queryType === 'commands' && Array.isArray(evt.data)) {
            setSlashCommands(evt.data as string[])
          } else if (evt.queryType === 'agents' && Array.isArray(evt.data)) {
            setAgents(evt.data as string[])
          }
          break
        }
        case 'rewind_result': {
          const evt = raw as { requestId?: string; result: unknown }
          if (evt.requestId) channelRef.current.handleResponse(evt.requestId, evt.result)
          break
        }
        case 'mcp_set_result': {
          const evt = raw as { requestId?: string; result: unknown }
          if (evt.requestId) channelRef.current.handleResponse(evt.requestId, evt.result)
          break
        }
        case 'error': {
          if (raw.message === 'replay_buffer_exhausted' && raw.fatal === false) {
            // No longer setting streamGap — binary source switch handles this
            break
          }
          console.error('[WS] fatal error:', raw.message)
          break
        }
      }
    },
    [startHeartbeat],
  )

  // --- WS close handler ---
  const handleWsClose = useCallback(
    (ws: WebSocket, event: CloseEvent) => {
      if (wsRef.current !== ws) return
      if (unmountedRef.current) return
      clearHeartbeat()
      clearRespondedCache()
      channelRef.current.handleDisconnect()

      // Connection replaced by a newer WS (e.g. another tab connected)
      if (event.code === 4001) {
        setSessionState('replaced')
        setIsLive(false)
        return
      }

      if ((NON_RECOVERABLE_CODES as ReadonlySet<number>).has(event.code)) {
        setSessionState('error')
        setIsLive(false)
        return
      }

      // Recoverable close: attempt reconnect with backoff
      if (reconnectAttemptRef.current >= MAX_RECONNECT_ATTEMPTS) {
        setSessionState('error')
        setIsLive(false)
        return
      }

      setSessionState('reconnecting')
      reconnectAttemptRef.current++
      const backoff = Math.min(
        INITIAL_BACKOFF_MS * 2 ** (reconnectAttemptRef.current - 1),
        MAX_BACKOFF_MS,
      )
      const sid = sessionId
      reconnectTimerRef.current = setTimeout(() => {
        if (unmountedRef.current || !sid) return
        openWs(sid)
      }, backoff)
    },
    [clearHeartbeat, sessionId],
  )

  // --- Open WS ---
  const openWs = useCallback(
    (sid: string, model?: string) => {
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }

      const params = new URLSearchParams({ sessionId: sid })
      if (model) params.set('model', model)
      const ws = new WebSocket(sidecarWsUrl(`/ws/chat/${sid}?${params.toString()}`))
      wsRef.current = ws

      ws.onopen = () => {
        if (wsRef.current !== ws) return
        setIsLive(true)
        reconnectAttemptRef.current = 0

        // Always replay buffered events — critical for new sessions where
        // initialMessage response may have been emitted before WS connected.
        // On first connect (lastSeq=-1), getAfter(-1) returns all buffered events.
        // On reconnect (lastSeq=N), getAfter(N) returns only missed events.
        ws.send(JSON.stringify({ type: 'resume', lastSeq: lastSeqRef.current }))

        // Drain any messages queued while WS was connecting.
        // pendingMessagesRef survives reconnects — messages queued before the initial
        // WS connect are preserved and drained on the first successful onopen.
        for (const msg of pendingMessagesRef.current) {
          ws.send(JSON.stringify(msg))
        }
        pendingMessagesRef.current = []
      }

      ws.onmessage = (event) => handleWsMessage(ws, event)
      ws.onclose = (event) => handleWsClose(ws, event)
      ws.onerror = () => {
        if (wsRef.current !== ws) return
      }
    },
    [handleWsMessage, handleWsClose],
  )

  // --- Check active sessions on sessionId change ---
  useEffect(() => {
    if (!sessionId) {
      setMsgState({ committed: [], pendingText: '' })
      setSessionState('idle')
      setControlId(null)
      setIsLive(false)
      setInitComplete(false)
      return
    }

    const sid = sessionId
    unmountedRef.current = false
    lastSeqRef.current = -1
    reconnectAttemptRef.current = 0
    setMsgState({ committed: [], pendingText: '' })

    let cancelled = false

    async function init() {
      // Check if session is active
      try {
        const res = await fetch('/api/sidecar/sessions')
        if (!cancelled && res.ok) {
          const data: { active: ActiveSession[] } = await res.json()
          const active = data.active.find((s) => s.sessionId === sid)
          if (!cancelled && active) {
            setControlId(active.controlId)
            // Signal that session IS initializing — gates suppressNotFound in useConversation
            setSessionState('initializing')
            // Always auto-connect for active sessions — ensures we get live events
            // even when session is idle (waiting_input). Bug fix: previously only
            // connected for processing states, missing events from idle sessions.
            openWs(sid)
          }
        }
      } catch {
        // Active check failed — session is history-only
      }
      if (!cancelled) setInitComplete(true)
    }

    init()

    return () => {
      cancelled = true
      unmountedRef.current = true
      setInitComplete(false)
      clearHeartbeat()
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current)
      wsRef.current?.close()
      wsRef.current = null
      pendingMessagesRef.current = [] // prevent stale messages replaying to wrong session
      resumingRef.current = false
      // NOTE: We do NOT terminate the SDK session here. React cleanup fires on
      // in-app navigation (e.g. /chat/X → /sessions), which would kill the session
      // and force a full re-resume when the user navigates back. Session termination
      // on page close/refresh is handled by the beforeunload listener below.
    }
  }, [sessionId, openWs, clearHeartbeat])

  // Terminate SDK session on page close/refresh (NOT on in-app navigation).
  // Uses keepalive so the request completes even after the page unloads.
  // Pattern: Jupyter kernel idle timeout / VS Code Remote SSH — session data
  // (JSONL) is preserved, only the live SDK connection is closed.
  useEffect(() => {
    const handleBeforeUnload = () => {
      if (controlIdRef.current) {
        fetch(`/api/sidecar/sessions/${controlIdRef.current}`, {
          method: 'DELETE',
          keepalive: true,
        }).catch(() => {})
      }
    }
    window.addEventListener('beforeunload', handleBeforeUnload)
    return () => window.removeEventListener('beforeunload', handleBeforeUnload)
  }, [])

  // --- Send function ---
  const send = useCallback((msg: Record<string, unknown>) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return
    ws.send(JSON.stringify(msg))
  }, [])

  // --- Connect and send (lazy WS connection + auto-resume) ---
  const connectAndSend = useCallback(
    (msg: Record<string, unknown>) => {
      const ws = wsRef.current
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(msg))
        return
      }
      // Queue the message for delivery on ws.onopen
      pendingMessagesRef.current.push(msg)
      // WS already CONNECTING — onopen will drain pending messages
      if (ws && ws.readyState === WebSocket.CONNECTING) return

      if (!ws && sessionId) {
        if (controlId) {
          // Have controlId — just open WS
          setSessionState('initializing')
          openWs(sessionId)
        } else if (!resumingRef.current) {
          // Dormant session — auto-resume, then connect.
          // Set visual state so input bar shows "connecting" instead of idle.
          setSessionState('initializing')
          // Include persisted permission mode so the session starts with the correct mode.
          // Check session-specific key first, then global last-used mode.
          let permissionMode: string | undefined
          try {
            permissionMode =
              localStorage.getItem(`claude-view:mode:${sessionId}`) ??
              localStorage.getItem('claude-view:last-mode') ??
              undefined
          } catch {
            /* noop */
          }
          resumingRef.current = true
          fetch(`/api/sidecar/sessions/${sessionId}/resume`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ permissionMode }),
          })
            .then((res) => {
              if (!res.ok) throw new Error(`Resume failed: ${res.status}`)
              return res.json()
            })
            .then((data: { controlId: string }) => {
              if (unmountedRef.current) return
              setControlId(data.controlId)
              setSessionState('initializing')
              // biome-ignore lint/style/noNonNullAssertion: sessionId checked at call site
              openWs(sessionId!)
            })
            .catch(() => {
              // Resume failed — clear pending messages so they don't leak
              pendingMessagesRef.current = []
            })
            .finally(() => {
              resumingRef.current = false
            })
        }
        // If resume already in progress, messages are queued and will drain on WS open
      }
    },
    [sessionId, controlId, openWs],
  )

  // --- Reconnect ---
  const reconnect = useCallback(() => {
    if (!sessionId) return
    reconnectAttemptRef.current = 0
    setSessionState('initializing')
    openWs(sessionId)
  }, [sessionId, openWs])

  // --- Resume ---
  const resume = useCallback(
    async (permissionMode?: string, model?: string) => {
      if (!sessionId) return

      try {
        const res = await fetch(`/api/sidecar/sessions/${sessionId}/resume`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ permissionMode, model }),
        })
        if (res.ok) {
          const data = await res.json()
          setControlId(data.controlId)
          setSessionState('initializing')
          openWs(sessionId, model)
        }
      } catch {
        // Resume failed
      }
    },
    [sessionId, openWs],
  )

  // Keep controlId ref in sync for cleanup closures (avoids stale closure capture)
  controlIdRef.current = controlId

  // Keep channel's send function in sync with the effective send
  channelRef.current.updateSend(isLive ? send : null)

  const clearPendingMessage = useCallback((text: string) => {
    pendingMessagesRef.current = pendingMessagesRef.current.filter(
      (m) => (m as { content?: string }).content !== text,
    )
  }, [])

  const effectiveSend = deriveEffectiveSend(isLive, controlId, sessionId, send, connectAndSend)

  return {
    blocks: msgState.committed, // Backward compat alias
    committedBlocks: msgState.committed,
    pendingText: msgState.pendingText,
    sessionState,
    controlId,
    send: effectiveSend,
    sendIfLive: isLive ? send : null,
    isLive,
    reconnect,
    resume,
    totalInputTokens,
    contextWindowSize,
    model,
    slashCommands,
    mcpServers,
    permissionMode,
    skills,
    agents,
    channel: channelRef.current,
    capabilities,
    clearPendingMessage,
    initComplete,
  }
}
