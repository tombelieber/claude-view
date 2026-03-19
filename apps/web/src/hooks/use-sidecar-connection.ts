import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { useCallback, useEffect, useRef, useState } from 'react'
import { wsUrl } from '../lib/ws-url'

export type ChatSessionStatus = 'active' | 'idle' | 'watching' | 'error' | 'ended'
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk'

export interface SidecarConnection {
  isLive: boolean
  status: ChatSessionStatus
  committedBlocks: ConversationBlock[]
  pendingText: string
  model: string
  permissionMode: PermissionMode
  contextTokens: number
  contextLimit: number
  contextPercent: number
  totalCost: number | null
  send: (msg: unknown) => void
  disconnect: () => void
}

interface MessageState {
  committed: ConversationBlock[]
  pendingText: string
}

const INITIAL_BACKOFF_MS = 1000
const MAX_BACKOFF_MS = 30_000
const MAX_RECONNECT_ATTEMPTS = 10

export function useSidecarConnection(
  sessionId: string,
  opts?: { skip?: boolean },
): SidecarConnection {
  const [msgState, setMsgState] = useState<MessageState>({ committed: [], pendingText: '' })
  const [isLive, setIsLive] = useState(false)
  const [status, setStatus] = useState<ChatSessionStatus>('idle')
  const [model, setModel] = useState('')
  const [permissionMode, setPermissionMode] = useState<PermissionMode>('default')
  const [contextTokens, setContextTokens] = useState(0)
  const [contextLimit, setContextLimit] = useState(0)
  const [contextPercent, setContextPercent] = useState(0)
  const [totalCost, setTotalCost] = useState<number | null>(null)

  const wsRef = useRef<WebSocket | null>(null)
  const unmountedRef = useRef(false)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const handleMessage = useCallback((event: MessageEvent) => {
    let raw: Record<string, unknown>
    try {
      raw = JSON.parse(event.data)
    } catch {
      return
    }

    switch (raw.type) {
      case 'blocks_snapshot': {
        const blocks = raw.blocks as ConversationBlock[]
        setMsgState({ committed: blocks, pendingText: '' })
        break
      }
      case 'blocks_update': {
        const blocks = raw.blocks as ConversationBlock[]
        setMsgState({ committed: blocks, pendingText: '' })
        break
      }
      case 'stream_delta': {
        if (raw.textDelta) {
          const delta = raw.textDelta as string
          setMsgState((prev) => ({ ...prev, pendingText: prev.pendingText + delta }))
        }
        break
      }
      case 'session_state': {
        const s = raw.status as ChatSessionStatus
        setStatus(s)
        break
      }
      case 'session_init': {
        if (raw.model) setModel(raw.model as string)
        if (raw.permissionMode) setPermissionMode(raw.permissionMode as PermissionMode)
        break
      }
      case 'mode_changed': {
        setPermissionMode(raw.mode as PermissionMode)
        break
      }
      case 'turn_complete':
      case 'turn_error': {
        if (raw.blocks) {
          const blocks = raw.blocks as ConversationBlock[]
          setMsgState({ committed: blocks, pendingText: '' })
        }
        if (raw.totalCostUsd != null) {
          setTotalCost(raw.totalCostUsd as number)
        }
        if (raw.contextTokens != null) {
          setContextTokens(raw.contextTokens as number)
        }
        if (raw.contextLimit != null) {
          setContextLimit(raw.contextLimit as number)
        }
        if (raw.contextPercent != null) {
          setContextPercent(raw.contextPercent as number)
        }
        break
      }
      case 'session_closed': {
        setStatus('ended')
        setIsLive(false)
        break
      }
    }
  }, [])

  const openWs = useCallback(
    (sid: string) => {
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }

      const ws = new WebSocket(wsUrl(`/ws/chat/${sid}`))
      wsRef.current = ws

      ws.onopen = () => {
        if (wsRef.current !== ws) return
        setIsLive(true)
        reconnectAttemptRef.current = 0
      }

      ws.onmessage = (event) => {
        if (wsRef.current !== ws) return
        handleMessage(event)
      }

      ws.onclose = (event) => {
        if (wsRef.current !== ws) return
        if (unmountedRef.current) return
        setIsLive(false)

        // Non-recoverable close
        if (event.code === 1000 || event.code === 4004) {
          setStatus('ended')
          return
        }

        // Recoverable: attempt reconnect with exponential backoff
        if (reconnectAttemptRef.current >= MAX_RECONNECT_ATTEMPTS) {
          setStatus('error')
          return
        }

        reconnectAttemptRef.current++
        const backoff = Math.min(
          INITIAL_BACKOFF_MS * 2 ** (reconnectAttemptRef.current - 1),
          MAX_BACKOFF_MS,
        )
        reconnectTimerRef.current = setTimeout(() => {
          if (unmountedRef.current) return
          openWs(sid)
        }, backoff)
      }

      ws.onerror = () => {
        // onclose will fire after onerror
      }
    },
    [handleMessage],
  )

  // Connect on mount (unless skip=true)
  useEffect(() => {
    if (opts?.skip || !sessionId) return

    unmountedRef.current = false
    reconnectAttemptRef.current = 0
    setMsgState({ committed: [], pendingText: '' })
    setStatus('idle')
    setIsLive(false)

    openWs(sessionId)

    return () => {
      unmountedRef.current = true
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current)
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [sessionId, opts?.skip, openWs])

  const send = useCallback((msg: unknown) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return
    ws.send(JSON.stringify(msg))
  }, [])

  const disconnect = useCallback(() => {
    if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current)
    wsRef.current?.close()
    wsRef.current = null
    setIsLive(false)
  }, [])

  return {
    isLive,
    status,
    committedBlocks: msgState.committed,
    pendingText: msgState.pendingText,
    model,
    permissionMode,
    contextTokens,
    contextLimit,
    contextPercent,
    totalCost,
    send,
    disconnect,
  }
}
