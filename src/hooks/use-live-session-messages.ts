import { useState, useCallback } from 'react'
import { parseRichMessage, type RichMessage } from '../components/live/RichPane'
import { useTerminalSocket, type ConnectionState } from './use-terminal-socket'
import type { HookEventItem } from '../components/live/action-log/types'

export interface UseLiveSessionMessagesResult {
  messages: RichMessage[]
  hookEvents: HookEventItem[]
  bufferDone: boolean
  connectionState: ConnectionState
}

/**
 * Manages a WebSocket connection for a live session and parses incoming
 * messages into RichMessage[].
 *
 * This is the "lifted" version of the logic previously inside
 * RichTerminalPane — by calling it in SessionDetailPanel, both the
 * Terminal tab and the (future) Log tab can share the same WebSocket
 * connection and message array.
 */
export function useLiveSessionMessages(sessionId: string, enabled: boolean): UseLiveSessionMessagesResult {
  const [messages, setMessages] = useState<RichMessage[]>([])
  const [hookEvents, setHookEvents] = useState<HookEventItem[]>([])
  const [bufferDone, setBufferDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
    // Try to parse as hook_event first (JSON with type field)
    try {
      const json = JSON.parse(data)
      if (json.type === 'hook_event') {
        setHookEvents((prev) => [...prev, {
          id: `hook-${prev.length}`,
          type: 'hook_event' as const,
          timestamp: json.timestamp,
          eventName: json.eventName,
          toolName: json.toolName,
          label: json.label,
          group: json.group,
          context: json.context,
        }])
        // Also push into RichMessage stream so verbose mode shows hook events
        setMessages((prev) => [...prev, {
          type: 'hook' as const,
          content: json.label,
          name: json.eventName,
          input: json.context,
          ts: json.timestamp,
        }])
        return
      }
    } catch {
      // Not JSON or not a hook_event — fall through to rich message parsing
    }

    const parsed = parseRichMessage(data)
    if (parsed) {
      setMessages((prev) => [...prev, parsed])
    }
  }, [])

  const handleConnectionChange = useCallback((state: ConnectionState) => {
    if (state === 'connected') {
      setBufferDone(true)
    }
  }, [])

  const { connectionState } = useTerminalSocket({
    sessionId,
    mode: 'rich',
    enabled,
    onMessage: handleMessage,
    onConnectionChange: handleConnectionChange,
  })

  return { messages, hookEvents, bufferDone, connectionState }
}
