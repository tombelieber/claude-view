import { useCallback } from 'react'
import { useSessionMutations } from '../../hooks/use-session-mutations'
import { trackFeatureAction } from '../../lib/telemetry-track'
import { ChatInputBar } from './ChatInputBar'

export function NewSessionInput({
  onSessionCreated,
}: { onSessionCreated: (sessionId: string) => void }) {
  const { createSession } = useSessionMutations()

  const handleSend = useCallback(
    async (message: string) => {
      const result = await createSession.mutateAsync({ initialMessage: message })
      trackFeatureAction('chat_message_sent')
      onSessionCreated(result.sessionId)
    },
    [onSessionCreated, createSession],
  )

  return <ChatInputBar onSend={handleSend} placeholder="What do you want to build?" />
}
