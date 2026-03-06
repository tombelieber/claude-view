// apps/web/src/hooks/use-session-control.ts
// Unified hook for session control lifecycle — owns resume, WS, messages, health.
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
import { type ControlStatus, useControlSession } from './use-control-session'

// ---------------------------------------------------------------------------
// Phase state machine
// ---------------------------------------------------------------------------

export type SessionPhase =
  | 'idle'
  | 'resuming'
  | 'connecting'
  | 'ready'
  | 'reconnecting'
  | 'completed'
  | 'error'

export type ConnectionHealth = 'ok' | 'degraded' | 'lost'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function nextLocalId(): string {
  return crypto.randomUUID()
}

function phaseToInputBarState(phase: SessionPhase, controlStatus: ControlStatus): InputBarState {
  switch (phase) {
    case 'idle':
      return 'dormant'
    case 'resuming':
      return 'resuming'
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
  send: (text: string) => void
  retry: (localId: string) => void
  respondPermission: (id: string, allowed: boolean) => void
  answerQuestion: (id: string, answers: Record<string, string>) => void
  approvePlan: (id: string, approved: boolean, feedback?: string) => void
  submitElicitation: (id: string, response: string) => void
}

export function useSessionControl(sessionId: string): UseSessionControlReturn {
  const [phase, setPhase] = useState<SessionPhase>('idle')
  const [controlId, setControlId] = useState<string | null>(null)
  const [messages, setMessages] = useState<ChatMessageWithStatus[]>([])
  const [error, setError] = useState<string | null>(null)

  // Refs for synchronous access (avoids stale closures)
  const pendingQueueRef = useRef<string[]>([])
  const messagesRef = useRef<ChatMessageWithStatus[]>([])
  const resumeInFlightRef = useRef(false)
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

  // Internal WS hook — only connects when controlId is set
  const controlSession = useControlSession(controlId)

  // ---------------------------------------------------------------------------
  // Phase transitions driven by controlSession.status
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (!controlId) return

    const s = controlSession.status
    if (s === 'active' || s === 'waiting_input') {
      setPhase('ready')
    } else if (s === 'reconnecting') {
      setPhase('reconnecting')
    } else if (s === 'completed') {
      setPhase('completed')
    } else if (s === 'error') {
      setPhase('error')
      setError(controlSession.error ?? 'Session error')
    } else if (s === 'disconnected' && controlId) {
      // WS hasn't opened yet — don't treat initial 'disconnected' as error
      // while we're still in 'connecting' phase (controlId was just set).
      if (phase === 'connecting') return
      setPhase('error')
      setError('Connection lost — session may have ended')
      // Mark all pending messages as failed
      if (pendingQueueRef.current.length > 0) {
        const pendingIds = new Set(pendingQueueRef.current)
        setMessages((prev) =>
          prev.map((m) =>
            pendingIds.has(m.localId) ? { ...m, status: 'failed' as MessageStatus } : m,
          ),
        )
        pendingQueueRef.current = []
      }
    }
  }, [controlId, controlSession.status, controlSession.error, phase])

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
      : controlSession.status === 'error' ||
          (controlSession.status === 'disconnected' && controlId != null)
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
  // Uses resumeInFlightRef (not phase state) to guard against double-resume.
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

      if ((phase === 'idle' || phase === 'error') && !resumeInFlightRef.current) {
        // Need to resume first — guard with ref to prevent double-resume
        resumeInFlightRef.current = true
        pendingQueueRef.current.push(localId)
        setPhase('resuming')

        fetch('/api/control/resume', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sessionId }),
        })
          .then(async (res) => {
            if (!res.ok) throw new Error(`Resume failed: ${res.status}`)
            const data = await res.json()
            setControlId(data.controlId)
            setPhase('connecting')
          })
          .catch(() => {
            setPhase('error')
            setError('Failed to resume session')
            setMessages((prev) =>
              prev.map((m) =>
                m.localId === localId ? { ...m, status: 'failed' as MessageStatus } : m,
              ),
            )
            pendingQueueRef.current = pendingQueueRef.current.filter((id) => id !== localId)
          })
          .finally(() => {
            resumeInFlightRef.current = false
          })
      } else if (phase === 'ready' && controlSession.status === 'waiting_input') {
        // Already connected — send immediately
        setMessages((prev) =>
          prev.map((m) =>
            m.localId === localId ? { ...m, status: 'sending' as MessageStatus } : m,
          ),
        )
        controlSession.sendMessage(text)
      } else {
        // Queue for later drain (e.g. during connecting/resuming)
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
    send,
    retry,
    respondPermission: controlSession.respondPermission,
    answerQuestion: controlSession.answerQuestion,
    approvePlan: controlSession.approvePlan,
    submitElicitation: controlSession.submitElicitation,
  }
}
