import { useQuery } from '@tanstack/react-query'
import type { ParsedSession } from '../types/generated'

// Re-export for backward compatibility with existing imports
export type { ToolCall, Message } from '../types/generated'

// Alias ParsedSession to SessionData for backward compatibility
export type SessionData = ParsedSession

/** Error subclass that carries the HTTP status code. */
export class HttpError extends Error {
  constructor(message: string, public readonly status: number) {
    super(message)
    this.name = 'HttpError'
  }
}

/** Type-safe check for a 404 HttpError. */
export function isNotFoundError(err: unknown): boolean {
  return err instanceof HttpError && err.status === 404
}

async function fetchSession(sessionId: string): Promise<SessionData> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/parsed`)
  if (!response.ok) {
    throw new HttpError('Failed to fetch session', response.status)
  }
  return response.json()
}

export function useSession(sessionId: string | null) {
  return useQuery({
    queryKey: ['session', sessionId],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchSession(sessionId)
    },
    enabled: !!sessionId,
    retry: (_, error) => !isNotFoundError(error),
  })
}
