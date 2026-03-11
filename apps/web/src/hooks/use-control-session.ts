// apps/web/src/hooks/use-control-session.ts
import { useCallback, useEffect, useReducer, useRef, useState } from 'react'
import { wsUrl } from '../lib/ws-url'
import type {
  AskUserQuestionMsg,
  AuthStatusMsg,
  ChatMessage,
  ContextCompactedMsg,
  ElicitationMsg,
  HookEventMsg,
  ModelUsageInfo,
  PermissionRequestMsg,
  PlanApprovalMsg,
  RateLimitMsg,
  ServerMessage,
  SessionInitMsg,
  TaskProgressMsg,
  TaskStartedMsg,
  ToolProgressMsg,
} from '../types/control'
import { CLOSE_CODES, type PermissionMode } from '../types/control'
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
  thinkingContent: string
  // Legacy fields kept for backwards compat with use-session-control consumers
  contextUsage: number
  turnCount: number
  sessionCost: number | null
  lastTurnCost: number | null
  tokenUsage: { input: number; output: number; cacheRead: number; cacheCreation: number } | null
  model: string | null
  contextWindow: number | null
  // New fields for 27-event protocol
  sessionInit: SessionInitMsg | null
  rateLimitStatus: RateLimitMsg | null
  activeTasks: Map<string, TaskStartedMsg | TaskProgressMsg>
  activeToolProgress: Map<string, ToolProgressMsg>
  contextCompaction: ContextCompactedMsg | null
  fastModeState: string | null
  hookEvents: HookEventMsg[]
  promptSuggestion: string | null
  modelUsage: Record<string, ModelUsageInfo>
  authStatus: AuthStatusMsg | null
  // Interactive card state
  toolPairMap: Map<
    string,
    {
      toolName: string
      toolInput: Record<string, unknown>
      result?: { output: string; isError: boolean }
      startTime: number
    }
  >
  permissionRequest: PermissionRequestMsg | null
  askQuestion: AskUserQuestionMsg | null
  planApproval: PlanApprovalMsg | null
  elicitation: ElicitationMsg | null
  error: string | null
}

function makeInitialUIState(): ControlSessionState {
  return {
    messages: [],
    streamingContent: '',
    streamingMessageId: '',
    thinkingContent: '',
    contextUsage: 0,
    turnCount: 0,
    sessionCost: null,
    lastTurnCost: null,
    tokenUsage: null,
    model: null,
    contextWindow: null,
    sessionInit: null,
    rateLimitStatus: null,
    activeTasks: new Map(),
    activeToolProgress: new Map(),
    contextCompaction: null,
    fastModeState: null,
    hookEvents: [],
    promptSuggestion: null,
    modelUsage: {},
    authStatus: null,
    toolPairMap: new Map(),
    permissionRequest: null,
    askQuestion: null,
    planApproval: null,
    elicitation: null,
    error: null,
  }
}

const INITIAL_BACKOFF_MS = 1000
const MAX_BACKOFF_MS = 30_000

function handleServerMessage(prev: ControlSessionState, msg: ServerMessage): ControlSessionState {
  switch (msg.type) {
    case 'assistant_text':
      return {
        ...prev,
        streamingContent: prev.streamingContent + msg.text,
        streamingMessageId: msg.messageId,
      }

    case 'assistant_thinking':
      return {
        ...prev,
        thinkingContent: prev.thinkingContent + msg.thinking,
        messages: [
          ...prev.messages,
          {
            role: 'thinking',
            content: msg.thinking,
            messageId: msg.messageId,
          },
        ],
      }

    case 'assistant_error':
      return { ...prev, error: msg.error }

    case 'turn_complete': {
      // Materialize any in-progress streaming content
      const newMessages = [...prev.messages]
      if (prev.streamingContent) {
        newMessages.push({
          role: 'assistant',
          content: prev.streamingContent,
          messageId: prev.streamingMessageId || undefined,
        })
      }
      // Derive contextUsage from modelUsage if available
      const firstModel = Object.keys(msg.modelUsage)[0]
      const firstUsage = firstModel ? msg.modelUsage[firstModel] : null
      const ctxWindow = firstUsage?.contextWindow ?? prev.contextWindow ?? 0
      const inputTokens = firstUsage?.inputTokens ?? 0
      const ctxPercent = ctxWindow > 0 ? (inputTokens / ctxWindow) * 100 : 0
      return {
        ...prev,
        messages: newMessages,
        streamingContent: '',
        streamingMessageId: '',
        thinkingContent: '',
        sessionCost: msg.totalCostUsd,
        lastTurnCost: msg.totalCostUsd,
        turnCount: msg.numTurns,
        modelUsage: msg.modelUsage,
        fastModeState: msg.fastModeState ?? prev.fastModeState,
        contextUsage: ctxPercent,
        contextWindow: ctxWindow || prev.contextWindow,
        model: firstModel ?? prev.model,
      }
    }

    case 'turn_error':
      return {
        ...prev,
        streamingContent: '',
        streamingMessageId: '',
        thinkingContent: '',
        sessionCost: msg.totalCostUsd,
        turnCount: msg.numTurns,
        modelUsage: msg.modelUsage,
        fastModeState: msg.fastModeState ?? prev.fastModeState,
        error: msg.errors.join('\n') || 'Turn error',
      }

    case 'tool_use_start': {
      const newMap = new Map(prev.toolPairMap)
      newMap.set(msg.toolUseId, {
        toolName: msg.toolName,
        toolInput: msg.toolInput,
        startTime: Date.now(),
      })
      return {
        ...prev,
        toolPairMap: newMap,
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
    }

    case 'tool_use_result': {
      const newMap = new Map(prev.toolPairMap)
      const existing = newMap.get(msg.toolUseId)
      if (existing) {
        newMap.set(msg.toolUseId, {
          ...existing,
          result: { output: msg.output, isError: msg.isError },
        })
      }
      return {
        ...prev,
        toolPairMap: newMap,
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
    }

    case 'tool_progress': {
      const newMap = new Map(prev.activeToolProgress)
      newMap.set(msg.toolUseId, msg)
      return { ...prev, activeToolProgress: newMap }
    }

    case 'tool_summary':
      return {
        ...prev,
        messages: [...prev.messages, { role: 'assistant', content: msg.summary }],
      }

    case 'session_init':
      return {
        ...prev,
        sessionInit: msg,
        model: msg.model,
      }

    case 'session_status':
      // New protocol: session_status only carries 'compacting' | null + optional permissionMode
      return prev

    case 'session_closed':
      // Terminal — sessionStatus tracking in the WS handler handles the status transition
      return prev

    case 'context_compacted':
      return { ...prev, contextCompaction: msg }

    case 'rate_limit':
      return { ...prev, rateLimitStatus: msg }

    case 'task_started': {
      const newTasks = new Map(prev.activeTasks)
      newTasks.set(msg.taskId, msg)
      return { ...prev, activeTasks: newTasks }
    }

    case 'task_progress': {
      const newTasks = new Map(prev.activeTasks)
      newTasks.set(msg.taskId, msg)
      return { ...prev, activeTasks: newTasks }
    }

    case 'task_notification': {
      const newTasks = new Map(prev.activeTasks)
      newTasks.delete(msg.taskId)
      return { ...prev, activeTasks: newTasks }
    }

    case 'hook_event':
      return { ...prev, hookEvents: [...prev.hookEvents, msg] }

    case 'auth_status':
      return { ...prev, authStatus: msg }

    case 'files_saved':
    case 'command_output':
    case 'stream_delta':
    case 'unknown_sdk_event':
      // Acknowledged but no UI state change needed
      return prev

    case 'prompt_suggestion':
      return { ...prev, promptSuggestion: msg.suggestion }

    case 'permission_request':
      // Dedup replayed requests
      if (prev.permissionRequest?.requestId === msg.requestId) return prev
      return { ...prev, permissionRequest: msg }

    case 'ask_question':
      // Dedup replayed requests
      if (prev.askQuestion?.requestId === msg.requestId) return prev
      return { ...prev, askQuestion: msg }

    case 'plan_approval':
      if (prev.planApproval?.requestId === msg.requestId) return prev
      return { ...prev, planApproval: msg }

    case 'elicitation':
      if (prev.elicitation?.requestId === msg.requestId) return prev
      return { ...prev, elicitation: msg }

    case 'elicitation_complete':
      // MCP server elicitation lifecycle signal — no UI state change needed.
      // The elicitation card is dismissed by the user via submitElicitation().
      return prev

    case 'error':
      return { ...prev, error: msg.message }

    default:
      return prev
  }
}

export function useControlSession(sessionId: string | null) {
  const [connState, dispatch] = useReducer(connectionReducer, { phase: 'idle' } as ConnectionState)
  const [ui, setUI] = useState<ControlSessionState>(makeInitialUIState)
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
      setUI(makeInitialUIState())
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
        setUI((prev) => handleServerMessage(prev, msg))

        // Track session sub-status from session_closed
        if (msg.type === 'session_closed') {
          setSessionStatus('completed')
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

        setUI((prev) => handleServerMessage(prev, msg))

        if (msg.type === 'session_closed') {
          setSessionStatus('completed')
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

  const setMode = useCallback(
    (mode: PermissionMode) => {
      sendRaw({ type: 'set_mode', mode })
    },
    [sendRaw],
  )

  return {
    status,
    messages: ui.messages,
    streamingContent: ui.streamingContent,
    streamingMessageId: ui.streamingMessageId,
    thinkingContent: ui.thinkingContent,
    // Legacy fields (backwards compat)
    contextUsage: ui.contextUsage,
    turnCount: ui.turnCount,
    sessionCost: ui.sessionCost,
    lastTurnCost: ui.lastTurnCost,
    tokenUsage: ui.tokenUsage,
    model: ui.model,
    contextWindow: ui.contextWindow,
    // New fields
    sessionInit: ui.sessionInit,
    rateLimitStatus: ui.rateLimitStatus,
    activeTasks: ui.activeTasks,
    activeToolProgress: ui.activeToolProgress,
    contextCompaction: ui.contextCompaction,
    fastModeState: ui.fastModeState,
    hookEvents: ui.hookEvents,
    promptSuggestion: ui.promptSuggestion,
    modelUsage: ui.modelUsage,
    authStatus: ui.authStatus,
    toolPairMap: ui.toolPairMap,
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
    setMode,
  }
}
