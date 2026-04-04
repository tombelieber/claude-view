/**
 * Multiplexed WebSocket hook for per-session data.
 *
 * Connects to /api/live/sessions/{id}/ws and receives typed frames
 * (blocks, raw terminal, SDK events, session state) over a single connection.
 *
 * Replaces:
 * - use-terminal-socket.ts (JSONL streaming)
 * - use-block-socket.ts (block accumulation)
 * - Sidecar WS relay in use-command-executor.ts (future)
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { wsUrl } from '../lib/ws-url'

// ── Types matching Rust SessionFrame ────────────────────────────────

type FrameMode = 'block' | 'terminal_raw' | 'sdk' | 'session_state'

interface ClientHandshake {
  modes: FrameMode[]
  scrollback?: { block?: number; raw?: number }
}

/** Discriminated union matching Rust SessionFrame. */
type SessionFrame =
  | { frame: 'handshake_ack'; session_id: string; modes: FrameMode[] }
  | { frame: 'block_delta'; [key: string]: unknown }
  | { frame: 'block_buffer_end' }
  | { frame: 'terminal_raw'; line: string }
  | { frame: 'terminal_buffer_end' }
  | { frame: 'sdk_event'; payload: unknown }
  | { frame: 'sdk_status'; connected: boolean }
  | { frame: 'session_state_update'; [key: string]: unknown }
  | { frame: 'pong' }
  | { frame: 'error'; message: string; code: string }

// ── Connection state ────────────────────────────────────────────────

export type ChannelState = 'connecting' | 'connected' | 'disconnected' | 'error'

const MAX_RECONNECT_ATTEMPTS = 10
const INITIAL_BACKOFF_MS = 1_000
const MAX_BACKOFF_MS = 30_000

// ── Hook options ────────────────────────────────────────────────────

export interface UseSessionChannelOptions {
  sessionId: string
  /** Which frame types to subscribe to. Defaults to ['block']. */
  modes?: FrameMode[]
  /** Block scrollback count. Default 50. */
  blockScrollback?: number
  /** Raw line scrollback count. Default 1000. */
  rawScrollback?: number
  /** Whether the connection should be active. */
  enabled: boolean
}

export interface UseSessionChannelResult {
  /** Accumulated conversation blocks (block mode). */
  blocks: ConversationBlock[]
  /** Whether initial scrollback buffer has been fully received. */
  bufferDone: boolean
  /** Raw terminal lines (terminal_raw mode). */
  rawLines: string[]
  /** Connection state. */
  connectionState: ChannelState
  /** Last error message from server. */
  error: string | null
  /** Whether SDK is connected (from sdk_status frames). */
  sdkConnected: boolean
}

/**
 * Multiplexed session WebSocket hook.
 *
 * Connects to a single WS endpoint and receives typed frames for blocks,
 * raw terminal data, SDK events, and session state — all through one connection.
 */
export function useSessionChannel(options: UseSessionChannelOptions): UseSessionChannelResult {
  const {
    sessionId,
    modes = ['block'],
    blockScrollback = 50,
    rawScrollback = 1000,
    enabled,
  } = options

  const [connectionState, setConnectionState] = useState<ChannelState>('disconnected')
  const [blockMap, setBlockMap] = useState<Map<string, ConversationBlock>>(new Map())
  const [bufferDone, setBufferDone] = useState(false)
  const [rawLines, setRawLines] = useState<string[]>([])
  const [error, setError] = useState<string | null>(null)
  const [sdkConnected, setSdkConnected] = useState(false)

  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const intentionalCloseRef = useRef(false)
  const unmountedRef = useRef(false)

  // Stable mode key for dependency tracking
  const modeKey = modes.sort().join(',')

  const connect = useCallback(() => {
    if (unmountedRef.current) return

    const url = wsUrl(`/api/live/sessions/${sessionId}/ws`)
    const ws = new WebSocket(url)
    wsRef.current = ws
    setConnectionState('connecting')

    ws.onopen = () => {
      reconnectAttemptRef.current = 0
      // Send handshake
      const handshake: ClientHandshake = {
        modes: modes as FrameMode[],
        scrollback: { block: blockScrollback, raw: rawScrollback },
      }
      ws.send(JSON.stringify(handshake))
    }

    ws.onmessage = (ev) => {
      let frame: SessionFrame
      try {
        frame = JSON.parse(ev.data)
      } catch {
        return
      }

      switch (frame.frame) {
        case 'handshake_ack':
          setConnectionState('connected')
          setError(null)
          break

        case 'block_delta': {
          // Extract the block data (everything except the 'frame' discriminator)
          const { frame: _f, ...blockData } = frame
          const block = blockData as unknown as ConversationBlock
          if (block.id && block.type) {
            setBlockMap((prev) => {
              const next = new Map(prev)
              next.set(block.id, block)
              return next
            })
          }
          break
        }

        case 'block_buffer_end':
          setBufferDone(true)
          break

        case 'terminal_raw':
          setRawLines((prev) => [...prev, frame.line])
          break

        case 'terminal_buffer_end':
          // Raw scrollback complete
          break

        case 'sdk_event':
          // Future: dispatch to SDK event handlers
          break

        case 'sdk_status':
          setSdkConnected(frame.connected)
          break

        case 'pong':
          break

        case 'error':
          setError(frame.message)
          break
      }
    }

    ws.onclose = (ev) => {
      wsRef.current = null
      if (intentionalCloseRef.current || unmountedRef.current) {
        setConnectionState('disconnected')
        return
      }
      // Auto-reconnect with exponential backoff
      if (reconnectAttemptRef.current < MAX_RECONNECT_ATTEMPTS) {
        const delay = Math.min(
          INITIAL_BACKOFF_MS * 2 ** reconnectAttemptRef.current,
          MAX_BACKOFF_MS,
        )
        reconnectAttemptRef.current++
        setConnectionState('connecting')
        reconnectTimerRef.current = setTimeout(connect, delay)
      } else {
        setConnectionState('error')
        setError(`Connection lost after ${MAX_RECONNECT_ATTEMPTS} retries (code ${ev.code})`)
      }
    }

    ws.onerror = () => {
      // onclose will fire after onerror — state handled there
    }
  }, [sessionId, modeKey, blockScrollback, rawScrollback])

  useEffect(() => {
    unmountedRef.current = false
    intentionalCloseRef.current = false

    if (!enabled) {
      intentionalCloseRef.current = true
      if (wsRef.current) {
        wsRef.current.onclose = null
        wsRef.current.close()
        wsRef.current = null
      }
      setConnectionState('disconnected')
      return
    }

    // Reset state on new connection
    setBlockMap(new Map())
    setBufferDone(false)
    setRawLines([])
    setError(null)
    setSdkConnected(false)

    connect()

    return () => {
      unmountedRef.current = true
      intentionalCloseRef.current = true
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current)
        reconnectTimerRef.current = null
      }
      if (wsRef.current) {
        wsRef.current.onclose = null
        wsRef.current.close()
        wsRef.current = null
      }
    }
  }, [enabled, connect])

  const blocks = useMemo(() => [...blockMap.values()], [blockMap])

  return {
    blocks,
    bufferDone,
    rawLines,
    connectionState,
    error,
    sdkConnected,
  }
}
