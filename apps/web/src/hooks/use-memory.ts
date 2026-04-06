import { useQuery } from '@tanstack/react-query'

// ── Types matching Rust MemoryEntry / MemoryIndex / ProjectMemoryGroup ──

export type MemoryType = 'user' | 'feedback' | 'project' | 'reference'

export interface MemoryEntry {
  name: string
  description: string
  memoryType: MemoryType
  body: string
  filename: string
  relativePath: string
  scope: string
  projectDir: string
  sizeBytes: number
  modifiedAt: number
}

export interface ProjectMemoryGroup {
  projectDir: string
  displayName: string
  count: number
  memories: MemoryEntry[]
}

export interface MemoryIndex {
  totalCount: number
  global: MemoryEntry[]
  projects: ProjectMemoryGroup[]
}

async function fetchMemoryIndex(): Promise<MemoryIndex> {
  const response = await fetch('/api/memory')
  if (!response.ok) {
    throw new Error(`Failed to fetch memory: ${await response.text()}`)
  }
  return response.json()
}

async function fetchMemoryFile(relativePath: string): Promise<MemoryEntry> {
  const response = await fetch(`/api/memory/file?path=${encodeURIComponent(relativePath)}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch memory file: ${await response.text()}`)
  }
  return response.json()
}

/**
 * Fetch all memory entries (global + per-project).
 * Stale time: 30s — memories change between sessions, not within.
 */
export function useMemoryIndex() {
  return useQuery({
    queryKey: ['memory-index'],
    queryFn: fetchMemoryIndex,
    staleTime: 30_000,
  })
}

/**
 * Fetch a single memory file by relative path.
 */
export function useMemoryFile(relativePath: string | null) {
  return useQuery({
    queryKey: ['memory-file', relativePath],
    queryFn: () => fetchMemoryFile(relativePath!),
    enabled: !!relativePath,
    staleTime: 60_000,
  })
}
