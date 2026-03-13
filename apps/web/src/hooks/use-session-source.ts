import { StreamAccumulator } from '@claude-view/shared/lib'
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { ModelUsageInfo, SequencedEvent } from '@claude-view/shared/types/sidecar-protocol'
import { useCallback, useEffect, useRef, useState } from 'react'
import { wsUrl } from '../lib/ws-url'
import { NON_RECOVERABLE_CODES } from '../types/control'

const INITIAL_BACKOFF_MS = 1000
const MAX_BACKOFF_MS = 30_000
const MAX_RECONNECT_ATTEMPTS = 10

export interface SessionSourceResult {
  blocks: ConversationBlock[]
  sessionState: string
  controlId: string | null
  send: ((msg: Record<string, unknown>) => void) | null
  isLive: boolean
  reconnect: () => void
  resume: (permissionMode?: string, model?: string) => Promise<void>
  totalInputTokens: number
  contextWindowSize: number
  /** True if session is known-active but WS not yet opened */
  canResumeLazy: boolean
  model: string
  slashCommands: string[]
  mcpServers: { name: string; status: string }[]
}

/** Exported for testing — determines which send function to use based on connection state. */
export function deriveEffectiveSend(
  isLive: boolean,
  controlId: string | null,
  send: ((msg: Record<string, unknown>) => void) | null,
  connectAndSend: (msg: Record<string, unknown>) => void,
): ((msg: Record<string, unknown>) => void) | null {
  if (isLive) return send // WS is open, use direct send
  if (controlId) return connectAndSend // Lazy resumable — queue + connect
  return null // Truly dormant — no send capability
}

/** Exported for testing — true when session has a controlId but WS not yet opened. */
export function deriveCanResumeLazy(controlId: string | null, isLive: boolean): boolean {
  return !!controlId && !isLive
}

export function useSessionSource(sessionId: string | undefined): SessionSourceResult {
  const [liveBlocks, setLiveBlocks] = useState<ConversationBlock[]>([])
  const [sessionState, setSessionState] = useState<string>('idle')
  const [controlId, setControlId] = useState<string | null>(null)
  const [isLive, setIsLive] = useState(false)
  const [totalInputTokens, setTotalInputTokens] = useState(0)
  const [contextWindowSize, setContextWindowSize] = useState(0)
  const [model, setModel] = useState('')
  const [slashCommands, setSlashCommands] = useState<string[]>([])
  const [mcpServers, setMcpServers] = useState<{ name: string; status: string }[]>([])

  const wsRef = useRef<WebSocket | null>(null)
  const accumulatorRef = useRef<StreamAccumulator>(new StreamAccumulator())
  const lastSeqRef = useRef(-1)
  const unmountedRef = useRef(false)
  const heartbeatTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const pongReceivedRef = useRef(true)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pendingMessagesRef = useRef<Record<string, unknown>[]>([])

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

  // --- Update live blocks from accumulator ---
  const syncBlocks = useCallback(() => {
    setLiveBlocks(accumulatorRef.current.getBlocks())
  }, [])

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

      accumulatorRef.current.push(raw as unknown as SequencedEvent)
      syncBlocks()

      // Track session state from events
      switch (raw.type) {
        case 'session_init': {
          setSessionState('waiting_input')
          const init = raw as unknown as {
            model?: string
            slashCommands?: string[]
            mcpServers?: { name: string; status: string }[]
          }
          if (init.model) setModel(init.model)
          if (init.slashCommands) setSlashCommands(init.slashCommands)
          if (init.mcpServers) setMcpServers(init.mcpServers)
          break
        }
        case 'session_status':
          if (raw.status === 'compacting') {
            setSessionState('compacting')
          }
          break
        case 'turn_complete':
        case 'turn_error': {
          setSessionState('waiting_input')
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
        case 'permission_request':
          setSessionState('waiting_permission')
          break
        case 'session_closed':
          setSessionState('closed')
          setIsLive(false)
          break
      }
    },
    [startHeartbeat, syncBlocks],
  )

  // --- WS close handler ---
  const handleWsClose = useCallback(
    (ws: WebSocket, event: CloseEvent) => {
      if (wsRef.current !== ws) return
      if (unmountedRef.current) return
      clearHeartbeat()

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

      setSessionState('reconnecting' as string)
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
      const ws = new WebSocket(wsUrl(`/api/control/connect?${params.toString()}`))
      wsRef.current = ws

      ws.onopen = () => {
        if (wsRef.current !== ws) return
        setIsLive(true)
        reconnectAttemptRef.current = 0

        if (lastSeqRef.current >= 0) {
          ws.send(JSON.stringify({ type: 'resume', lastSeq: lastSeqRef.current }))
        }

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
      setLiveBlocks([])
      setSessionState('idle')
      setControlId(null)
      setIsLive(false)
      return
    }

    const sid = sessionId
    unmountedRef.current = false
    accumulatorRef.current = new StreamAccumulator()
    lastSeqRef.current = -1
    reconnectAttemptRef.current = 0

    let cancelled = false

    async function init() {
      // Check if session is active
      try {
        const res = await fetch('/api/control/sessions')
        if (!cancelled && res.ok) {
          const sessions: { controlId: string; sessionId: string }[] = await res.json()
          const active = sessions.find((s) => s.sessionId === sid)
          if (!cancelled && active) {
            setControlId(active.controlId)
            // Don't open WS yet — wait for user to send first message (lazy connect)
          }
        }
      } catch {
        // Active check failed — session is history-only
      }
    }

    init()

    return () => {
      cancelled = true
      unmountedRef.current = true
      clearHeartbeat()
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current)
      wsRef.current?.close()
      wsRef.current = null
      pendingMessagesRef.current = [] // prevent stale messages replaying to wrong session
    }
  }, [sessionId, openWs, clearHeartbeat])

  // --- Send function ---
  const send = useCallback((msg: Record<string, unknown>) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return
    ws.send(JSON.stringify(msg))
  }, [])

  // --- Connect and send (lazy WS connection) ---
  const connectAndSend = useCallback(
    (msg: Record<string, unknown>) => {
      const ws = wsRef.current
      if (ws && ws.readyState === WebSocket.OPEN) {
        // Already connected — send directly
        ws.send(JSON.stringify(msg))
        return
      }
      // Queue the message for delivery on ws.onopen
      pendingMessagesRef.current.push(msg)
      // !ws means no WS at all (not merely CONNECTING) — CONNECTING WS will drain the queue in onopen.
      if (!ws && sessionId && controlId) {
        openWs(sessionId)
      }
    },
    [sessionId, controlId, openWs],
  )

  // --- Reconnect ---
  const reconnect = useCallback(() => {
    if (!sessionId) return
    reconnectAttemptRef.current = 0
    openWs(sessionId)
  }, [sessionId, openWs])

  // --- Resume ---
  const resume = useCallback(
    async (permissionMode?: string, model?: string) => {
      if (!sessionId) return

      try {
        const res = await fetch('/api/control/sessions/resume', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sessionId, permissionMode, model }),
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

  const effectiveSend = deriveEffectiveSend(isLive, controlId, send, connectAndSend)
  const canResumeLazy = deriveCanResumeLazy(controlId, isLive)

  return {
    blocks: liveBlocks, // Only live/accumulator blocks — history comes from useHistoryBlocks
    sessionState,
    controlId,
    send: effectiveSend,
    isLive,
    reconnect,
    resume,
    totalInputTokens,
    contextWindowSize,
    canResumeLazy,
    model,
    slashCommands,
    mcpServers,
  }
}
