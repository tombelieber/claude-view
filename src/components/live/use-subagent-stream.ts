import { useCallback, useState } from 'react'
import { useTerminalSocket, type ConnectionState } from '../../hooks/use-terminal-socket'
import { parseRichMessage, type RichMessage } from './RichPane'

export interface UseSubAgentStreamOptions {
  sessionId: string
  agentId: string | null
  enabled: boolean
  onMessage: (data: string) => void
}

export interface UseSubAgentStreamResult {
  connectionState: ConnectionState
  messages: RichMessage[]
  bufferDone: boolean
  reconnect: () => void
}

/**
 * Hook for streaming a sub-agent's conversation over WebSocket.
 *
 * Wraps `useTerminalSocket` and embeds the sub-agent path into the sessionId
 * so the constructed URL becomes:
 *   /api/live/sessions/<sessionId>/subagents/<agentId>/terminal
 *
 * Parses each incoming message into a RichMessage and accumulates them
 * for display in a RichPane. Detects the `buffer_end` signal to mark
 * when historical scrollback has finished loading.
 */
export function useSubAgentStream(options: UseSubAgentStreamOptions): UseSubAgentStreamResult {
  const { sessionId, agentId, enabled, onMessage } = options
  const [messages, setMessages] = useState<RichMessage[]>([])
  const [bufferDone, setBufDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
    // Check for buffer_end signal
    try {
      const parsed = JSON.parse(data)
      if (parsed.type === 'buffer_end') {
        setBufDone(true)
        // Don't return — let useTerminalSocket handle the state transition,
        // but still forward to the consumer
        onMessage(data)
        return
      }
    } catch { /* not JSON, continue */ }

    const rich = parseRichMessage(data)
    if (rich) {
      setMessages((prev) => [...prev, rich])
    }
    onMessage(data)
  }, [onMessage])

  // useTerminalSocket constructs: /api/live/sessions/${sessionId}/terminal
  // By embedding the subagent path IN the sessionId, the URL becomes:
  // /api/live/sessions/abc123/subagents/a951849/terminal
  const { connectionState, reconnect } = useTerminalSocket({
    sessionId: agentId ? `${sessionId}/subagents/${agentId}` : sessionId,
    mode: 'rich',
    scrollback: 100_000,
    enabled: enabled && agentId !== null,
    onMessage: handleMessage,
  })

  return { connectionState, messages, bufferDone, reconnect }
}
