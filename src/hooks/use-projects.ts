import { useQuery } from '@tanstack/react-query'
import type { ProjectInfo, SessionInfo } from '../types/generated'

// Re-export for backward compatibility with existing imports
export type { ProjectInfo, SessionInfo } from '../types/generated'

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
