import { useQuery } from '@tanstack/react-query'
import type { ParsedSession } from '../types/generated'

// Re-export for backward compatibility with existing imports
export type { ToolCall, Message } from '../types/generated'

// Alias ParsedSession to SessionData for backward compatibility
export type SessionData = ParsedSession

async function fetchSession(projectDir: string, sessionId: string): Promise<SessionData> {
  const response = await fetch(`/api/session/${encodeURIComponent(projectDir)}/${encodeURIComponent(sessionId)}`)
  if (!response.ok) {
    throw new Error('Failed to fetch session')
  }
  return response.json()
}

export function useSession(projectDir: string | null, sessionId: string | null) {
  return useQuery({
    queryKey: ['session', projectDir, sessionId],
    queryFn: () => {
      if (!projectDir || !sessionId) {
        throw new Error('projectDir and sessionId are required')
      }
      return fetchSession(projectDir, sessionId)
    },
    enabled: !!projectDir && !!sessionId,
  })
}
