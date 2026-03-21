import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { useTrackEvent } from './use-track-event'

interface UseSendHandlerOptions {
  sessionId: string | undefined
  selectedModel: string
  permMode: string
  onSessionCreated?: (sessionId: string) => void
  sendMessage: (text: string) => void
}

/**
 * Handles the send flow for ChatSession:
 * - If no session exists: POST /api/sidecar/sessions to create one, then transition the panel
 * - If session exists: delegate to actions.sendMessage
 */
export function useSendHandler({
  sessionId,
  selectedModel,
  permMode,
  onSessionCreated,
  sendMessage,
}: UseSendHandlerOptions) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const trackEvent = useTrackEvent()

  return useCallback(
    (text: string) => {
      if (!sessionId) {
        trackEvent('chat_started')
        fetch('/api/sidecar/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            model: selectedModel,
            initialMessage: text,
            permissionMode: permMode,
          }),
        })
          .then((r) => r.json())
          .then((data) => {
            if (data.sessionId) {
              queryClient.invalidateQueries({ queryKey: ['chat-sidebar-sessions'] })
              if (onSessionCreated) {
                onSessionCreated(data.sessionId)
              } else {
                navigate(`/chat/${data.sessionId}`)
              }
            } else {
              toast.error('Failed to create session', {
                description: data.error || 'No session ID returned',
              })
            }
          })
          .catch(() => {
            toast.error('Failed to create session')
          })
        return
      }
      sendMessage(text)
    },
    [
      sessionId,
      sendMessage,
      navigate,
      selectedModel,
      permMode,
      queryClient,
      trackEvent,
      onSessionCreated,
    ],
  )
}
