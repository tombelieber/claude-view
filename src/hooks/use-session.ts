import { useQuery } from '@tanstack/react-query'

export interface ToolCall {
  name: string
  count: number
}

export interface Message {
  role: 'user' | 'assistant'
  content: string
  timestamp?: string
  toolCalls?: ToolCall[]
}

export interface SessionData {
  messages: Message[]
  metadata: {
    totalMessages: number
    toolCallCount: number
  }
}

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
