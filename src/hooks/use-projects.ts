import { useQuery } from '@tanstack/react-query'

export interface SessionInfo {
  id: string
  project: string
  projectPath: string
  filePath: string
  modifiedAt: number  // Unix timestamp in seconds
  sizeBytes: number
  preview: string
  lastMessage: string
  filesTouched: string[]
  skillsUsed: string[]
  toolCounts: {
    edit: number
    read: number
    bash: number
    write: number
  }
  messageCount: number   // Total messages in session
  turnCount: number      // Human→assistant exchange pairs
}

export interface ProjectInfo {
  name: string
  displayName: string  // Just the project folder name (e.g., "claude-view")
  path: string
  sessions: SessionInfo[]
  activeCount: number  // Number of sessions active in the last hour
}

async function fetchProjects(): Promise<ProjectInfo[]> {
  const response = await fetch('/api/projects')
  if (!response.ok) {
    throw new Error('Failed to fetch projects')
  }
  const data = await response.json()

  // API returns modifiedAt as ISO 8601 strings; convert to Unix seconds
  for (const project of data) {
    for (const session of project.sessions) {
      if (typeof session.modifiedAt === 'string') {
        session.modifiedAt = Math.floor(new Date(session.modifiedAt).getTime() / 1000)
      }
    }
  }

  return data
}

export function useProjects() {
  return useQuery({
    queryKey: ['projects'],
    queryFn: fetchProjects,
  })
}
