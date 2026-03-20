import { useCallback } from 'react'
import { ChatInputBar } from './ChatInputBar'

export function NewSessionInput({
  onSessionCreated,
}: { onSessionCreated: (sessionId: string) => void }) {
  const handleSend = useCallback(
    async (message: string) => {
      try {
        const res = await fetch('/api/sidecar/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ initialMessage: message }),
        })
        if (!res.ok) {
          throw new Error(`Failed to start session: ${res.status}`)
        }
        const data = await res.json()
        if (!data.sessionId) {
          throw new Error('No sessionId returned from server')
        }
        onSessionCreated(data.sessionId)
      } catch (err) {
        console.error('Failed to start session:', err)
      }
    },
    [onSessionCreated],
  )

  return <ChatInputBar onSend={handleSend} placeholder="What do you want to build?" />
}
