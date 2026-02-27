// apps/web/src/hooks/use-control-session.ts
import { useCallback, useEffect, useRef, useState } from 'react'
import { wsUrl } from '../lib/ws-url'
import type { ChatMessage, PermissionRequestMsg, ServerMessage } from '../types/control'

export type ControlStatus =
  | 'connecting'
  | 'active'
  | 'waiting_input'
  | 'waiting_permission'
  | 'completed'
  | 'error'
  | 'disconnected'
  | 'reconnecting'

interface ControlSessionState {
  status: ControlStatus
  messages: ChatMessage[]
  streamingContent: string
  streamingMessageId: string
  contextUsage: number
  turnCount: number
  sessionCost: number
  lastTurnCost: number
  permissionRequest: PermissionRequestMsg | null
  error: string | null
}

const initialState: ControlSessionState = {
  status: 'connecting',
  messages: [],
  streamingContent: '',
  streamingMessageId: '',
  contextUsage: 0,
  turnCount: 0,
  sessionCost: 0,
  lastTurnCost: 0,
  permissionRequest: null,
  error: null,
}

export function useControlSession(controlId: string | null) {
  const [state, setState] = useState<ControlSessionState>(initialState)
  const wsRef = useRef<WebSocket | null>(null)
  const intentionalCloseRef = useRef(false)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const unmountedRef = useRef(false)
  const MAX_RECONNECT_ATTEMPTS = 10
  const INITIAL_BACKOFF_MS = 1000
  const MAX_BACKOFF_MS = 30_000

  useEffect(() => {
    if (!controlId) return
    unmountedRef.current = false
    intentionalCloseRef.current = false

    function connect() {
      // Clean up previous WS before creating new one (prevents leaked connections on reconnect)
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }
      const ws = new WebSocket(wsUrl(`/api/control/sessions/${controlId}/stream`))
      wsRef.current = ws

      ws.onmessage = (event) => {
        // Stale guard per CLAUDE.md rules
        if (wsRef.current !== ws) return

        const msg: ServerMessage = JSON.parse(event.data)

        setState((prev) => {
          switch (msg.type) {
            case 'assistant_chunk':
              return {
                ...prev,
                streamingContent: prev.streamingContent + msg.content,
                streamingMessageId: msg.messageId,
                status: 'active',
              }

            case 'assistant_done':
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
                status: 'waiting_input',
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
              return {
                ...prev,
                permissionRequest: msg,
                status: 'waiting_permission',
              }

            case 'session_status':
              return {
                ...prev,
                status: msg.status,
                contextUsage: msg.contextUsage,
                turnCount: msg.turnCount,
              }

            case 'error':
              return {
                ...prev,
                status: msg.fatal ? 'error' : prev.status,
                error: msg.message,
              }

            case 'pong':
              return prev // no state change

            default:
              return prev
          }
        })
      }

      // Non-recoverable close codes — don't attempt reconnect
      const NON_RECOVERABLE_CODES = [4004, 4500] // session not found, server shutdown

      ws.onclose = (event) => {
        if (wsRef.current !== ws) return // stale guard
        if (unmountedRef.current) return // unmount guard
        const canReconnect =
          !intentionalCloseRef.current &&
          reconnectAttemptRef.current < MAX_RECONNECT_ATTEMPTS &&
          !NON_RECOVERABLE_CODES.includes(event.code)
        if (canReconnect) {
          const backoff = Math.min(
            INITIAL_BACKOFF_MS * 2 ** reconnectAttemptRef.current,
            MAX_BACKOFF_MS,
          )
          reconnectTimerRef.current = setTimeout(connect, backoff)
          reconnectAttemptRef.current++
          setState((prev) => ({ ...prev, status: 'reconnecting' as ControlStatus }))
        } else {
          setState((prev) => ({
            ...prev,
            status: prev.status === 'completed' ? 'completed' : 'disconnected',
          }))
        }
      }

      ws.onopen = () => {
        if (wsRef.current !== ws) return // stale guard
        reconnectAttemptRef.current = 0 // reset on successful connect
        // Don't set any session status here — leave as 'connecting'.
        // The sidecar sends an initial 'session_status' message immediately
        // after WS handshake which will set the correct status
        // ('waiting_input', 'active', etc.). Setting status here would
        // race with that message and could show wrong state briefly.
      }

      ws.onerror = () => {
        if (wsRef.current !== ws) return
        setState((prev) => ({ ...prev, error: 'WebSocket connection error' }))
      }
    } // end connect()

    connect()

    // Close WS BEFORE nulling the ref -- otherwise the stale guard in
    // onclose fires (wsRef.current !== ws) and skips the state update.
    return () => {
      unmountedRef.current = true
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current)
      intentionalCloseRef.current = true
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [controlId])

  const sendMessage = useCallback((content: string) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return

    ws.send(JSON.stringify({ type: 'user_message', content }))

    setState((prev) => ({
      ...prev,
      messages: [...prev.messages, { role: 'user', content }],
      status: 'active',
    }))
  }, [])

  const respondPermission = useCallback((requestId: string, allowed: boolean) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return

    ws.send(JSON.stringify({ type: 'permission_response', requestId, allowed }))

    setState((prev) => ({
      ...prev,
      permissionRequest: null,
      status: 'active',
    }))
  }, [])

  return { ...state, sendMessage, respondPermission }
}
