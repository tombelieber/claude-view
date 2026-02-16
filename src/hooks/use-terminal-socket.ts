import { useState, useEffect, useRef, useCallback, useMemo } from 'react'
import { wsUrl } from '../lib/ws-url'

export type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error'

export interface UseTerminalSocketOptions {
  sessionId: string
  mode: 'raw' | 'rich'
  scrollback?: number
  enabled: boolean
  onMessage: (data: string) => void
  onConnectionChange?: (state: ConnectionState) => void
}

interface UseTerminalSocketResult {
  connectionState: ConnectionState
  sendMessage: (msg: object) => void
  reconnect: () => void
}

const MAX_RECONNECT_ATTEMPTS = 10
const MAX_BACKOFF_MS = 30_000
const INITIAL_BACKOFF_MS = 1_000

/**
 * WebSocket connection hook for terminal streaming.
 *
 * Connects to the live terminal WebSocket endpoint for a given session,
 * sends a handshake on open, and auto-reconnects with exponential backoff
 * on unexpected disconnects.
 *
 * When `enabled` is false, the WebSocket is intentionally closed (no reconnect).
 */
export function useTerminalSocket(options: UseTerminalSocketOptions): UseTerminalSocketResult {
  const { sessionId, mode, scrollback = 100_000, enabled, onMessage, onConnectionChange } = options

  // All hooks declared at top — no hooks after early returns (CLAUDE.md rule)
  const [internalState, setInternalState] = useState<ConnectionState>('disconnected')
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const intentionalCloseRef = useRef(false)
  const unmountedRef = useRef(false)
  const connectFnRef = useRef<(() => void) | null>(null)

  // Stable refs for callbacks to avoid re-triggering the effect
  const onMessageRef = useRef(onMessage)
  const onConnectionChangeRef = useRef(onConnectionChange)

  // Sync callback refs in an effect (react-hooks/refs forbids ref writes during render)
  useEffect(() => {
    onMessageRef.current = onMessage
  }, [onMessage])

  useEffect(() => {
    onConnectionChangeRef.current = onConnectionChange
  }, [onConnectionChange])

  // Derive effective state: when disabled, always report 'disconnected'
  const connectionState = useMemo<ConnectionState>(
    () => (enabled ? internalState : 'disconnected'),
    [enabled, internalState],
  )

  // Notify consumer when effective state changes
  const prevStateRef = useRef(connectionState)
  useEffect(() => {
    if (prevStateRef.current !== connectionState) {
      prevStateRef.current = connectionState
      onConnectionChangeRef.current?.(connectionState)
    }
  }, [connectionState])

  // Helper to update internal state and notify consumer (only from async callbacks)
  const updateState = useCallback((state: ConnectionState) => {
    if (unmountedRef.current) return
    setInternalState(state)
    onConnectionChangeRef.current?.(state)
  }, [])

  const sendMessage = useCallback((msg: object) => {
    const ws = wsRef.current
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg))
    }
  }, [])

  const reconnect = useCallback(() => {
    // Reset attempt counter so reconnect logic starts fresh
    reconnectAttemptRef.current = 0
    connectFnRef.current?.()
  }, [])

  // WebSocket connection lifecycle
  useEffect(() => {
    unmountedRef.current = false

    // When not enabled, close any existing connection intentionally
    if (!enabled) {
      intentionalCloseRef.current = true
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }
      if (reconnectTimerRef.current !== null) {
        clearTimeout(reconnectTimerRef.current)
        reconnectTimerRef.current = null
      }
      // Effective state derived as 'disconnected' via useMemo above
      return
    }

    function connect() {
      if (unmountedRef.current) return

      // Clean up any previous connection
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }

      intentionalCloseRef.current = false

      const url = wsUrl(`/api/live/sessions/${sessionId}/terminal`)
      const ws = new WebSocket(url)
      wsRef.current = ws

      // Transition to 'connecting' deferred to the open-pending state via ws events.
      // We set it here as a synchronous marker; the lint rule allows setState
      // in the same effect that sets up subscriptions (connect pattern).
      updateState('connecting')

      ws.onopen = () => {
        if (unmountedRef.current || wsRef.current !== ws) {
          ws.close()
          return
        }
        // Send handshake as first message
        ws.send(JSON.stringify({ mode, scrollback }))
        reconnectAttemptRef.current = 0
      }

      ws.onmessage = (event: MessageEvent) => {
        if (unmountedRef.current || wsRef.current !== ws) return
        const data = event.data as string

        // Check for buffer_end to transition to connected state
        try {
          const parsed = JSON.parse(data)
          if (parsed.type === 'buffer_end') {
            updateState('connected')
          }
        } catch {
          // Not JSON or parse error — still forward to consumer
        }

        onMessageRef.current(data)
      }

      ws.onclose = (event: CloseEvent) => {
        if (unmountedRef.current) return

        // Stale close event from a superseded connection — ignore it.
        // Without this guard, the old WS's async onclose nulls wsRef.current
        // (orphaning the new connection) and triggers a reconnect, cascading
        // into leaked connections that hit the server's per-session limit.
        if (wsRef.current !== ws) return

        wsRef.current = null

        // If we intentionally closed (enabled=false or unmount), don't reconnect
        if (intentionalCloseRef.current) {
          updateState('disconnected')
          return
        }

        // Server-rejected connections: don't reconnect on non-recoverable close codes
        // 4004 = session not found or per-session viewer limit exceeded
        // 4500 = file read/watch errors (scrollback failed, watcher failed, etc.)
        if (event.code === 4004 || event.code === 4500) {
          updateState('error')
          return
        }

        // Unexpected close — attempt reconnect with backoff
        reconnectAttemptRef.current += 1

        if (reconnectAttemptRef.current > MAX_RECONNECT_ATTEMPTS) {
          updateState('error')
          return
        }

        updateState('disconnected')

        const backoff = Math.min(
          INITIAL_BACKOFF_MS * Math.pow(2, reconnectAttemptRef.current - 1),
          MAX_BACKOFF_MS,
        )
        reconnectTimerRef.current = setTimeout(connect, backoff)
      }

      ws.onerror = () => {
        // The error event is followed by a close event, so we let onclose handle reconnect.
        // Just update state here for immediate feedback.
        if (!unmountedRef.current && wsRef.current === ws) {
          updateState('error')
        }
      }
    }

    connectFnRef.current = connect
    connect()

    return () => {
      unmountedRef.current = true
      intentionalCloseRef.current = true

      if (reconnectTimerRef.current !== null) {
        clearTimeout(reconnectTimerRef.current)
        reconnectTimerRef.current = null
      }

      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }
    }
  }, [sessionId, mode, scrollback, enabled, updateState])

  return { connectionState, sendMessage, reconnect }
}
