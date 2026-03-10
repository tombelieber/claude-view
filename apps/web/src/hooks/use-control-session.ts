// apps/web/src/hooks/use-control-session.ts
import { useCallback, useEffect, useReducer, useRef, useState } from 'react'
import { wsUrl } from '../lib/ws-url'
import type {
  AskUserQuestionMsg,
  ChatMessage,
  ElicitationMsg,
  PermissionRequestMsg,
  PlanApprovalMsg,
  ServerMessage,
} from '../types/control'
import { CLOSE_CODES } from '../types/control'
import { type ConnectionState, connectionReducer } from './connection-reducer'

export type ControlStatus =
  | 'idle'
  | 'connecting'
  | 'active'
  | 'waiting_input'
  | 'waiting_permission'
  | 'reconnecting'
  | 'completed'
  | 'fatal'
  | 'failed'

interface ControlSessionState {
  messages: ChatMessage[]
  streamingContent: string
  streamingMessageId: string
  contextUsage: number
  turnCount: number
  sessionCost: number | null
  lastTurnCost: number | null
  permissionRequest: PermissionRequestMsg | null
  askQuestion: AskUserQuestionMsg | null
  planApproval: PlanApprovalMsg | null
  elicitation: ElicitationMsg | null
  error: string | null
}

const initialUIState: ControlSessionState = {
  messages: [],
  streamingContent: '',
  streamingMessageId: '',
  contextUsage: 0,
  turnCount: 0,
  sessionCost: null,
  lastTurnCost: null,
  permissionRequest: null,
  askQuestion: null,
  planApproval: null,
  elicitation: null,
  error: null,
}

const INITIAL_BACKOFF_MS = 1000
const MAX_BACKOFF_MS = 30_000

export function useControlSession(sessionId: string | null) {
  const [connState, dispatch] = useReducer(connectionReducer, { phase: 'idle' } as ConnectionState)
  const [ui, setUI] = useState<ControlSessionState>(initialUIState)
  const [sessionStatus, setSessionStatus] = useState<string>('idle')

  const wsRef = useRef<WebSocket | null>(null)
  const unmountedRef = useRef(false)
  const heartbeatTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const pongReceivedRef = useRef(true)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // --- Heartbeat management ---
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
          // No pong since last ping — connection dead
          const ws = wsRef.current
          if (ws && ws.readyState === WebSocket.OPEN) {
            ws.close(CLOSE_CODES.HEARTBEAT_TIMEOUT, 'heartbeat_timeout')
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

  // --- Connect effect ---
  useEffect(() => {
    if (!sessionId) {
      setUI(initialUIState)
      setSessionStatus('idle')
      dispatch({ type: 'reset' })
      return
    }

    unmountedRef.current = false
    dispatch({ type: 'connect', sessionId })

    function openWs() {
      // Clean up previous WS
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }

      const ws = new WebSocket(wsUrl(`/api/control/connect?sessionId=${sessionId}`))
      wsRef.current = ws

      ws.onopen = () => {
        if (wsRef.current !== ws) return // stale guard
        dispatch({ type: 'ws_open' })

        // On reconnect, send resume with lastSeq
        // connState is captured via closure but we need the latest value
        // We'll handle this in the reconnect effect instead
      }

      ws.onmessage = (event) => {
        if (wsRef.current !== ws) return // stale guard

        let raw: Record<string, unknown>
        try {
          raw = JSON.parse(event.data)
        } catch {
          return // malformed frame — ignore
        }

        // heartbeat_config has NO seq — intercept before reducer
        if (raw.type === 'heartbeat_config') {
          startHeartbeat(raw.intervalMs as number)
          return
        }

        // pong — reset heartbeat counter, no reducer dispatch needed
        if (raw.type === 'pong') {
          pongReceivedRef.current = true
          return
        }

        // All other messages have seq from emitSequenced
        const seq: number = (raw.seq as number) ?? -1
        const msg = raw as unknown as ServerMessage
        dispatch({ type: 'ws_message', msg, seq })

        // Accumulate UI state
        setUI((prev) => {
          switch (msg.type) {
            case 'assistant_chunk':
              return {
                ...prev,
                streamingContent: prev.streamingContent + msg.content,
                streamingMessageId: msg.messageId,
              }

            case 'assistant_done':
              // Guard: skip materializing when no chunks preceded this done event.
              // The server may send assistant_done with zero chunks (e.g. empty turn).
              if (!prev.streamingContent) {
                return {
                  ...prev,
                  streamingContent: '',
                  streamingMessageId: '',
                  sessionCost: msg.totalCost,
                  lastTurnCost: msg.cost,
                }
              }
              return {
                ...prev,
                messages: [
                  ...prev.messages,
                  {
                    role: 'assistant',
                    content: prev.streamingContent,
                    messageId: msg.messageId,
                    usage: msg.usage,
                  },
                ],
                streamingContent: '',
                streamingMessageId: '',
                sessionCost: msg.totalCost,
                lastTurnCost: msg.cost,
              }

            case 'tool_use_start':
              return {
                ...prev,
                messages: [
                  ...prev.messages,
                  {
                    role: 'tool_use',
                    toolName: msg.toolName,
                    toolInput: msg.toolInput,
                    toolUseId: msg.toolUseId,
                  },
                ],
              }

            case 'tool_use_result':
              return {
                ...prev,
                messages: [
                  ...prev.messages,
                  {
                    role: 'tool_result',
                    toolUseId: msg.toolUseId,
                    output: msg.output,
                    isError: msg.isError,
                  },
                ],
              }

            case 'permission_request':
              // Dedup replayed requests
              if (prev.permissionRequest?.requestId === msg.requestId) return prev
              return { ...prev, permissionRequest: msg }

            case 'ask_user_question':
              // Dedup replayed requests
              if (prev.askQuestion?.requestId === msg.requestId) return prev
              return { ...prev, askQuestion: msg }

            case 'plan_approval':
              if (prev.planApproval?.requestId === msg.requestId) return prev
              return { ...prev, planApproval: msg }

            case 'elicitation':
              if (prev.elicitation?.requestId === msg.requestId) return prev
              return { ...prev, elicitation: msg }

            case 'session_status':
              return {
                ...prev,
                contextUsage: msg.contextUsage,
                turnCount: msg.turnCount,
              }

            case 'error':
              return { ...prev, error: msg.message }

            default:
              return prev
          }
        })

        // Track session sub-status separately for ControlStatus derivation
        if (msg.type === 'session_status') {
          setSessionStatus(msg.status)
        }
      }

      ws.onclose = (event) => {
        if (wsRef.current !== ws) return // stale guard
        if (unmountedRef.current) return
        clearHeartbeat()
        dispatch({ type: 'ws_close', code: event.code, reason: event.reason })
      }

      ws.onerror = () => {
        if (wsRef.current !== ws) return
        dispatch({ type: 'ws_error', error: 'WebSocket connection error' })
      }
    }

    openWs()

    return () => {
      unmountedRef.current = true
      clearHeartbeat()
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current)
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [sessionId, startHeartbeat, clearHeartbeat])

  // --- Reconnect effect: schedule reconnect when state enters 'reconnecting' ---
  // Destructure attempt/lastSeq so the linter sees explicit deps.
  // The early return guarantees these are only read when phase === 'reconnecting'.
  const reconnectAttempt = connState.phase === 'reconnecting' ? connState.attempt : 0
  const reconnectLastSeq = connState.phase === 'reconnecting' ? connState.lastSeq : -1

  useEffect(() => {
    if (connState.phase !== 'reconnecting') return
    if (!sessionId) return

    const backoff = Math.min(INITIAL_BACKOFF_MS * 2 ** (reconnectAttempt - 1), MAX_BACKOFF_MS)

    reconnectTimerRef.current = setTimeout(() => {
      if (unmountedRef.current) return
      // Clean up and create new WS
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }

      const ws = new WebSocket(wsUrl(`/api/control/connect?sessionId=${sessionId}`))
      wsRef.current = ws

      ws.onopen = () => {
        if (wsRef.current !== ws) return
        dispatch({ type: 'ws_open' })

        // Send resume to replay missed events
        if (reconnectLastSeq >= 0) {
          ws.send(JSON.stringify({ type: 'resume', lastSeq: reconnectLastSeq }))
        }
      }

      ws.onmessage = (event) => {
        if (wsRef.current !== ws) return

        let raw: Record<string, unknown>
        try {
          raw = JSON.parse(event.data)
        } catch {
          return // malformed frame — ignore
        }

        if (raw.type === 'heartbeat_config') {
          startHeartbeat(raw.intervalMs as number)
          return
        }
        if (raw.type === 'pong') {
          pongReceivedRef.current = true
          return
        }

        const seq: number = (raw.seq as number) ?? -1
        const msg = raw as unknown as ServerMessage
        dispatch({ type: 'ws_message', msg, seq })

        setUI((prev) => {
          switch (msg.type) {
            case 'assistant_chunk':
              return {
                ...prev,
                streamingContent: prev.streamingContent + msg.content,
                streamingMessageId: msg.messageId,
              }
            case 'assistant_done':
              // Guard: skip materializing when no chunks preceded this done event.
              // The server may send assistant_done with zero chunks (e.g. empty turn).
              if (!prev.streamingContent) {
                return {
                  ...prev,
                  streamingContent: '',
                  streamingMessageId: '',
                  sessionCost: msg.totalCost,
                  lastTurnCost: msg.cost,
                }
              }
              return {
                ...prev,
                messages: [
                  ...prev.messages,
                  {
                    role: 'assistant',
                    content: prev.streamingContent,
                    messageId: msg.messageId,
                    usage: msg.usage,
                  },
                ],
                streamingContent: '',
                streamingMessageId: '',
                sessionCost: msg.totalCost,
                lastTurnCost: msg.cost,
              }
            case 'tool_use_start':
              return {
                ...prev,
                messages: [
                  ...prev.messages,
                  {
                    role: 'tool_use',
                    toolName: msg.toolName,
                    toolInput: msg.toolInput,
                    toolUseId: msg.toolUseId,
                  },
                ],
              }
            case 'tool_use_result':
              return {
                ...prev,
                messages: [
                  ...prev.messages,
                  {
                    role: 'tool_result',
                    toolUseId: msg.toolUseId,
                    output: msg.output,
                    isError: msg.isError,
                  },
                ],
              }
            case 'permission_request':
              if (prev.permissionRequest?.requestId === msg.requestId) return prev
              return { ...prev, permissionRequest: msg }
            case 'ask_user_question':
              if (prev.askQuestion?.requestId === msg.requestId) return prev
              return { ...prev, askQuestion: msg }
            case 'plan_approval':
              if (prev.planApproval?.requestId === msg.requestId) return prev
              return { ...prev, planApproval: msg }
            case 'elicitation':
              if (prev.elicitation?.requestId === msg.requestId) return prev
              return { ...prev, elicitation: msg }
            case 'session_status':
              return { ...prev, contextUsage: msg.contextUsage, turnCount: msg.turnCount }
            case 'error':
              return { ...prev, error: msg.message }
            default:
              return prev
          }
        })

        if (msg.type === 'session_status') {
          setSessionStatus(msg.status)
        }
      }

      ws.onclose = (event) => {
        if (wsRef.current !== ws) return
        if (unmountedRef.current) return
        clearHeartbeat()
        dispatch({ type: 'ws_close', code: event.code, reason: event.reason })
      }

      ws.onerror = () => {
        if (wsRef.current !== ws) return
        dispatch({ type: 'ws_error', error: 'WebSocket connection error' })
      }
    }, backoff)

    return () => {
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current)
        reconnectTimerRef.current = null
      }
    }
  }, [
    connState.phase,
    reconnectAttempt,
    reconnectLastSeq,
    sessionId,
    startHeartbeat,
    clearHeartbeat,
  ])

  // --- Derive exported status ---
  const status: ControlStatus =
    connState.phase === 'active'
      ? (sessionStatus as ControlStatus)
      : (connState.phase as ControlStatus)

  // --- Send callbacks ---
  const sendMessage = useCallback((content: string) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return
    ws.send(JSON.stringify({ type: 'user_message', content }))
    setUI((prev) => ({
      ...prev,
      messages: [...prev.messages, { role: 'user', content }],
    }))
  }, [])

  const sendRaw = useCallback((msg: Record<string, unknown>) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return
    ws.send(JSON.stringify(msg))
  }, [])

  const respondPermission = useCallback((requestId: string, allowed: boolean) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return
    ws.send(JSON.stringify({ type: 'permission_response', requestId, allowed }))
    setUI((prev) => ({ ...prev, permissionRequest: null }))
  }, [])

  const answerQuestion = useCallback(
    (requestId: string, answers: Record<string, string>) => {
      sendRaw({ type: 'question_response', requestId, answers })
      setUI((prev) => ({ ...prev, askQuestion: null }))
    },
    [sendRaw],
  )

  const approvePlan = useCallback(
    (requestId: string, approved: boolean, feedback?: string) => {
      sendRaw({ type: 'plan_response', requestId, approved, feedback })
      setUI((prev) => ({ ...prev, planApproval: null }))
    },
    [sendRaw],
  )

  const submitElicitation = useCallback(
    (requestId: string, response: string) => {
      sendRaw({ type: 'elicitation_response', requestId, response })
      setUI((prev) => ({ ...prev, elicitation: null }))
    },
    [sendRaw],
  )

  return {
    status,
    messages: ui.messages,
    streamingContent: ui.streamingContent,
    streamingMessageId: ui.streamingMessageId,
    contextUsage: ui.contextUsage,
    turnCount: ui.turnCount,
    sessionCost: ui.sessionCost,
    lastTurnCost: ui.lastTurnCost,
    permissionRequest: ui.permissionRequest,
    askQuestion: ui.askQuestion,
    planApproval: ui.planApproval,
    elicitation: ui.elicitation,
    error: ui.error,
    fatalCode: connState.phase === 'fatal' ? (connState.code ?? null) : null,
    sendMessage,
    sendRaw,
    respondPermission,
    answerQuestion,
    approvePlan,
    submitElicitation,
  }
}
