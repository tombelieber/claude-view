import { useCallback, useState } from 'react'
import { type ConnectionState, useTerminalSocket } from '../../hooks/use-terminal-socket'
import type { ControlCallbacks } from '../../types/control-callbacks'
import { type RichMessage, RichPane, parseRichMessage } from './RichPane'

interface RichTerminalPaneProps {
  sessionId: string
  isVisible: boolean
  verboseMode: boolean
  controlCallbacks?: ControlCallbacks
}

/**
 * Wraps useTerminalSocket + RichPane for rich mode.
 * Manages its own WebSocket connection and parses messages into RichMessage[].
 */
export function RichTerminalPane({
  sessionId,
  isVisible,
  verboseMode,
  controlCallbacks,
}: RichTerminalPaneProps) {
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

  useTerminalSocket({
    sessionId,
    mode: 'rich',
    enabled: isVisible,
    onMessage: handleMessage,
    onConnectionChange: handleConnectionChange,
  })

  return (
    <RichPane
      messages={messages}
      isVisible={isVisible}
      verboseMode={verboseMode}
      bufferDone={bufferDone}
      controlCallbacks={controlCallbacks}
    />
  )
}
