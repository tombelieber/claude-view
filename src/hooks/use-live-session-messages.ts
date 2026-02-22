import { useState, useCallback, useRef } from 'react'
import { parseRichMessage, type RichMessage } from '../components/live/RichPane'
import { useTerminalSocket, type ConnectionState } from './use-terminal-socket'
import type { ActionCategory, HookEventItem } from '../components/live/action-log/types'

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
  const lastToolCategoryRef = useRef<ActionCategory | undefined>()

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
        // Insert hook event at correct chronological position
        // (scrollback replay sends hook events after all regular messages,
        //  so append-only would place them at the end instead of inline)
        setMessages((prev) => {
          const newMsg: RichMessage = {
            type: 'progress' as const,
            content: `Hook: ${json.eventName} — ${json.label}`,
            ts: json.timestamp,
            category: 'hook' as ActionCategory,
            metadata: {
              type: 'hook_event',
              _hookEvent: {
                id: `hook-${prev.length}`,
                type: 'hook_event' as const,
                timestamp: json.timestamp,
                eventName: json.eventName,
                toolName: json.toolName,
                label: json.label,
                group: json.group,
                context: json.context,
              },
            },
          }
          const ts = json.timestamp
          // Fast path: no timestamp or empty array — just append
          if (!ts || ts <= 0 || prev.length === 0) return [...prev, newMsg]
          // Fast path: timestamp >= last message — append (common for live events)
          const lastTs = prev[prev.length - 1].ts ?? 0
          if (ts >= lastTs) return [...prev, newMsg]
          // Slow path: binary search for correct insertion point
          let lo = 0, hi = prev.length
          while (lo < hi) {
            const mid = (lo + hi) >>> 1
            if ((prev[mid].ts ?? 0) <= ts) lo = mid + 1
            else hi = mid
          }
          const result = prev.slice()
          result.splice(lo, 0, newMsg)
          return result
        })
        return
      }
    } catch {
      // Not JSON or not a hook_event — fall through to rich message parsing
    }

    const parsed = parseRichMessage(data)
    if (parsed) {
      if (parsed.type === 'tool_use' && parsed.category) {
        lastToolCategoryRef.current = parsed.category
      } else if (parsed.type === 'tool_result') {
        parsed.category = lastToolCategoryRef.current
      }
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
