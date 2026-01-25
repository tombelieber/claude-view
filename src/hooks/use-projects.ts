import { useQuery } from '@tanstack/react-query'

export interface SessionInfo {
  id: string
  project: string
  projectPath: string
  filePath: string
  modifiedAt: string // ISO date string from JSON
  sizeBytes: number
  preview: string
}

export interface ProjectInfo {
  name: string
  displayName: string  // Just the project folder name (e.g., "claude-view")
  path: string
  sessions: SessionInfo[]
}

async function fetchProjects(): Promise<ProjectInfo[]> {
  const response = await fetch('/api/projects')
  if (!response.ok) {
    throw new Error('Failed to fetch projects')
  }
  return response.json()
}

export function useProjects() {
  return useQuery({
    queryKey: ['projects'],
    queryFn: fetchProjects,
  })
}
