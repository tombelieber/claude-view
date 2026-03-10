// apps/web/src/hooks/use-session-control.ts
// Unified hook for session control lifecycle — owns WS, messages, health.
import { useCallback, useEffect, useRef, useState } from 'react'
import type { InputBarState } from '../components/chat/ChatInputBar'
import type {
  AskUserQuestionMsg,
  ChatMessageWithStatus,
  ElicitationMsg,
  MessageStatus,
  PermissionRequestMsg,
  PlanApprovalMsg,
} from '../types/control'
import { CLOSE_CODES } from '../types/control'
import { type ControlStatus, useControlSession } from './use-control-session'

// ---------------------------------------------------------------------------
// Phase state machine
// ---------------------------------------------------------------------------

export type SessionPhase = 'idle' | 'connecting' | 'ready' | 'reconnecting' | 'completed' | 'error'

export type ConnectionHealth = 'ok' | 'degraded' | 'lost'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function nextLocalId(): string {
  return crypto.randomUUID()
}

function controlStatusToSessionPhase(status: ControlStatus): SessionPhase {
  switch (status) {
    case 'idle':
      return 'idle'
    case 'connecting':
      return 'connecting'
    case 'active':
    case 'waiting_input':
    case 'waiting_permission':
      return 'ready'
    case 'reconnecting':
      return 'reconnecting'
    case 'completed':
      return 'completed'
    case 'fatal':
    case 'failed':
      return 'error'
    default:
      return 'idle'
  }
}

function phaseToInputBarState(phase: SessionPhase, controlStatus: ControlStatus): InputBarState {
  switch (phase) {
    case 'idle':
      return 'dormant'
    case 'connecting':
      return 'connecting'
    case 'reconnecting':
      return 'reconnecting'
    case 'completed':
      return 'completed'
    case 'error':
      return 'dormant' // allow retry
    case 'ready':
      // Derive from WS control status
      switch (controlStatus) {
        case 'active':
          return 'streaming'
        case 'waiting_input':
          return 'active'
        case 'waiting_permission':
          return 'waiting_permission'
        case 'completed':
          return 'completed'
        default:
          return 'active'
      }
  }
}

// Timeout for optimistic messages (design requirement: 30s)
const MESSAGE_TIMEOUT_MS = 30_000

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export interface UseSessionControlReturn {
  phase: SessionPhase
  inputBarState: InputBarState
  messages: ChatMessageWithStatus[]
  connectionHealth: ConnectionHealth
  streamingContent: string
  contextPercent: number
  sessionCost: number | null
  lastTurnCost: number | null
  permissionRequest: PermissionRequestMsg | null
  askQuestion: AskUserQuestionMsg | null
  planApproval: PlanApprovalMsg | null
  elicitation: ElicitationMsg | null
  error: string | null
  fatalCode: number | null
  errorMessage: string | null
  send: (text: string) => void
  retry: (localId: string) => void
  respondPermission: (id: string, allowed: boolean) => void
  answerQuestion: (id: string, answers: Record<string, string>) => void
  approvePlan: (id: string, approved: boolean, feedback?: string) => void
  submitElicitation: (id: string, response: string) => void
}

export function useSessionControl(sessionId: string): UseSessionControlReturn {
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null)
  const [messages, setMessages] = useState<ChatMessageWithStatus[]>([])
  const [error, setError] = useState<string | null>(null)

  // Refs for synchronous access (avoids stale closures)
  const pendingQueueRef = useRef<string[]>([])
  const messagesRef = useRef<ChatMessageWithStatus[]>([])
  const messageTimeoutsRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())

  // Keep messagesRef in sync
  useEffect(() => {
    messagesRef.current = messages
  }, [messages])

  // Clean up timeouts on unmount
  useEffect(() => {
    return () => {
      for (const timer of messageTimeoutsRef.current.values()) {
        clearTimeout(timer)
      }
    }
  }, [])

  // Internal WS hook — only connects when activeSessionId is set
  const controlSession = useControlSession(activeSessionId)

  // ---------------------------------------------------------------------------
  // Phase derived from controlSession.status (no more local phase state)
  // ---------------------------------------------------------------------------
  const phase = controlStatusToSessionPhase(controlSession.status)

  // ---------------------------------------------------------------------------
  // Error side effects — set error message and fail pending messages
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (controlSession.status !== 'fatal' && controlSession.status !== 'failed') return
    setError(controlSession.error ?? 'Connection lost')
    if (pendingQueueRef.current.length > 0) {
      const pendingIds = new Set(pendingQueueRef.current)
      setMessages((prev) =>
        prev.map((m) =>
          pendingIds.has(m.localId) ? { ...m, status: 'failed' as MessageStatus } : m,
        ),
      )
      pendingQueueRef.current = []
    }
  }, [controlSession.status, controlSession.error])

  // ---------------------------------------------------------------------------
  // Drain pending messages when WS reaches waiting_input
  // Side-effect (sendMessage) is OUTSIDE the state updater — React 19 safe.
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (phase !== 'ready' || controlSession.status !== 'waiting_input') return
    if (pendingQueueRef.current.length === 0) return

    const localId = pendingQueueRef.current.shift()!
    const msg = messagesRef.current.find((m) => m.localId === localId)
    if (!msg?.content) return

    setMessages((prev) =>
      prev.map((m) => (m.localId === localId ? { ...m, status: 'sending' as MessageStatus } : m)),
    )
    // Side-effect OUTSIDE updater — safe for StrictMode
    controlSession.sendMessage(msg.content)
  }, [phase, controlSession.status, controlSession.sendMessage])

  // ---------------------------------------------------------------------------
  // Mark optimistic/sending messages as "sent" when assistant starts responding.
  // Guard: bail early if no messages need updating (avoids new array on every chunk).
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (controlSession.streamingContent.length === 0) return
    setMessages((prev) => {
      const hasOptimistic = prev.some(
        (m) => m.role === 'user' && (m.status === 'optimistic' || m.status === 'sending'),
      )
      if (!hasOptimistic) return prev // same reference — no re-render
      return prev.map((m) =>
        m.role === 'user' && (m.status === 'optimistic' || m.status === 'sending')
          ? { ...m, status: 'sent' as MessageStatus }
          : m,
      )
    })
  }, [controlSession.streamingContent])

  // ---------------------------------------------------------------------------
  // Connection health
  // ---------------------------------------------------------------------------
  const connectionHealth: ConnectionHealth =
    phase === 'reconnecting'
      ? 'degraded'
      : controlSession.status === 'fatal' || controlSession.status === 'failed'
        ? 'lost'
        : 'ok'

  // ---------------------------------------------------------------------------
  // Helper: schedule timeout for an optimistic message
  // ---------------------------------------------------------------------------
  const scheduleTimeout = useCallback((localId: string) => {
    const timer = setTimeout(() => {
      messageTimeoutsRef.current.delete(localId)
      setMessages((prev) =>
        prev.map((m) =>
          m.localId === localId && (m.status === 'optimistic' || m.status === 'sending')
            ? { ...m, status: 'failed' as MessageStatus }
            : m,
        ),
      )
      pendingQueueRef.current = pendingQueueRef.current.filter((id) => id !== localId)
    }, MESSAGE_TIMEOUT_MS)
    messageTimeoutsRef.current.set(localId, timer)
  }, [])

  const clearMessageTimeout = useCallback((localId: string) => {
    const timer = messageTimeoutsRef.current.get(localId)
    if (timer) {
      clearTimeout(timer)
      messageTimeoutsRef.current.delete(localId)
    }
  }, [])

  // Clear timeouts when messages resolve to 'sent' or 'failed'.
  // Early-exit: skip iteration if no active timeouts.
  useEffect(() => {
    if (messageTimeoutsRef.current.size === 0) return
    for (const msg of messages) {
      if (msg.status === 'sent' || msg.status === 'failed') {
        clearMessageTimeout(msg.localId)
      }
    }
  }, [messages, clearMessageTimeout])

  // ---------------------------------------------------------------------------
  // send()
  // ---------------------------------------------------------------------------
  const send = useCallback(
    (text: string) => {
      const localId = nextLocalId()
      const optimisticMsg: ChatMessageWithStatus = {
        role: 'user',
        content: text,
        localId,
        status: 'optimistic',
        createdAt: Date.now(),
      }
      setMessages((prev) => [...prev, optimisticMsg])
      setError(null)
      scheduleTimeout(localId)

      if (phase === 'idle' || phase === 'error') {
        // Trigger WS connection — sets activeSessionId which triggers useControlSession
        pendingQueueRef.current.push(localId)
        setActiveSessionId(sessionId)
      } else if (phase === 'ready' && controlSession.status === 'waiting_input') {
        // Already connected — send immediately
        setMessages((prev) =>
          prev.map((m) =>
            m.localId === localId ? { ...m, status: 'sending' as MessageStatus } : m,
          ),
        )
        controlSession.sendMessage(text)
      } else {
        // Queue for later drain (connecting phase)
        pendingQueueRef.current.push(localId)
      }
    },
    [phase, sessionId, controlSession.status, controlSession.sendMessage, scheduleTimeout],
  )

  // ---------------------------------------------------------------------------
  // retry()
  // Side-effects (setTimeout, send) are OUTSIDE the state updater — React 19 safe.
  // ---------------------------------------------------------------------------
  const retry = useCallback(
    (localId: string) => {
      const msg = messagesRef.current.find((m) => m.localId === localId)
      if (!msg || msg.status !== 'failed') return

      const content = msg.content
      // Remove the failed message first
      setMessages((prev) => prev.filter((m) => m.localId !== localId))
      // Then re-send — send() creates a fresh optimistic message
      setTimeout(() => send(content), 0)
    },
    [send],
  )

  // ---------------------------------------------------------------------------
  // Derived state
  // ---------------------------------------------------------------------------
  const inputBarState = phaseToInputBarState(phase, controlSession.status)

  const fatalCode = controlSession.fatalCode

  // Derive a user-friendly error message from the fatal close code
  let errorMessage: string | null = null
  if (fatalCode === CLOSE_CODES.SESSION_NOT_FOUND) {
    errorMessage = 'Session no longer available'
  } else if (fatalCode === CLOSE_CODES.SIDECAR_UNAVAILABLE) {
    errorMessage = 'Sidecar unavailable \u2014 is Claude Code running?'
  } else if (controlSession.status === 'fatal' || controlSession.status === 'failed') {
    errorMessage = controlSession.error ?? null
  }

  // ---------------------------------------------------------------------------
  // Absorb WS-received messages into the local messages array (in order).
  //
  // controlSession.messages contains ALL messages from the WS (user echoes from
  // sendMessage() + assistant_done materializations + tool_use_start + tool_use_result).
  // We track how many we've absorbed so we only push new arrivals, preserving
  // chronological interleaving with local optimistic user messages.
  //
  // User-role messages from WS are skipped — the local optimistic copy is canonical
  // (it has status/localId/createdAt lifecycle tracking).
  // ---------------------------------------------------------------------------
  const absorbedCountRef = useRef(0)

  useEffect(() => {
    const wsMessages = controlSession.messages
    if (wsMessages.length <= absorbedCountRef.current) return

    const newMessages = wsMessages.slice(absorbedCountRef.current)
    absorbedCountRef.current = wsMessages.length

    // Convert non-user WS messages to ChatMessageWithStatus and append
    const toAbsorb: ChatMessageWithStatus[] = []
    for (const m of newMessages) {
      // Skip user echoes — local optimistic message is the canonical copy
      if (m.role === 'user') continue
      toAbsorb.push({
        ...m,
        content: m.content ?? '',
        localId: m.messageId ?? `ws-${absorbedCountRef.current}`,
        status: 'sent' as MessageStatus,
        createdAt: Date.now(),
      })
    }

    if (toAbsorb.length > 0) {
      setMessages((prev) => [...prev, ...toAbsorb])
    }
  }, [controlSession.messages])

  // Reset absorbed count when WS messages array is cleared (session change/reconnect)
  useEffect(() => {
    if (controlSession.messages.length === 0) {
      absorbedCountRef.current = 0
    }
  }, [controlSession.messages])

  return {
    phase,
    inputBarState,
    messages,
    connectionHealth,
    streamingContent: controlSession.streamingContent,
    contextPercent: Math.round(controlSession.contextUsage),
    sessionCost: controlSession.sessionCost,
    lastTurnCost: controlSession.lastTurnCost,
    permissionRequest: controlSession.permissionRequest,
    askQuestion: controlSession.askQuestion,
    planApproval: controlSession.planApproval,
    elicitation: controlSession.elicitation,
    error,
    fatalCode,
    errorMessage,
    send,
    retry,
    respondPermission: controlSession.respondPermission,
    answerQuestion: controlSession.answerQuestion,
    approvePlan: controlSession.approvePlan,
    submitElicitation: controlSession.submitElicitation,
  }
}
