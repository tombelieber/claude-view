import { useState, useCallback } from 'react'
import { parseRichMessage, type RichMessage } from '../components/live/RichPane'
import { useTerminalSocket, type ConnectionState } from './use-terminal-socket'

export interface UseLiveSessionMessagesResult {
  messages: RichMessage[]
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
  const [bufferDone, setBufferDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
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

  return { messages, bufferDone, connectionState }
}
