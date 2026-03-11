import { useQuery } from '@tanstack/react-query'
import { useCallback, useMemo } from 'react'
import type { IdeDetectResponse } from '../types/generated/IdeDetectResponse'
import type { IdeInfo } from '../types/generated/IdeInfo'
import { useLocalStorage } from './use-local-storage'

const IDE_STORAGE_KEY = 'claude-view-preferred-ide'

async function fetchIdeDetect(): Promise<IdeDetectResponse> {
  const res = await fetch('/api/ide/detect')
  if (!res.ok) throw new Error(`IDE detect failed: ${res.status}`)
  return res.json()
}

async function postOpenInIde(ide: string, projectPath: string, filePath?: string): Promise<void> {
  const res = await fetch('/api/ide/open', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ide, projectPath, filePath }),
  })
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: `HTTP ${res.status}` }))
    throw new Error(body.error || `Failed to open IDE: ${res.status}`)
  }
}

export function useIdePreference() {
  const { data } = useQuery({
    queryKey: ['ide-detect'],
    queryFn: fetchIdeDetect,
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  })

  const availableIdes = data?.available ?? []
  const [storedIdeId, setStoredIdeId] = useLocalStorage<string | null>(IDE_STORAGE_KEY, null)

  const preferredIde = useMemo<IdeInfo | null>(() => {
    if (availableIdes.length === 0) return null
    if (storedIdeId) {
      const found = availableIdes.find((ide) => ide.id === storedIdeId)
      if (found) return found
    }
    return availableIdes[0]
  }, [availableIdes, storedIdeId])

  const setPreferredIde = useCallback(
    (id: string) => {
      setStoredIdeId(id)
    },
    [setStoredIdeId],
  )

  const openProject = useCallback(
    async (projectPath: string) => {
      if (!preferredIde) return
      await postOpenInIde(preferredIde.id, projectPath)
    },
    [preferredIde],
  )

  const openFile = useCallback(
    async (projectPath: string, filePath: string) => {
      if (!preferredIde) return
      await postOpenInIde(preferredIde.id, projectPath, filePath)
    },
    [preferredIde],
  )

  // Escape hatch: open with an explicit IDE id, bypassing the preferredIde closure.
  const openWithIde = useCallback(async (ideId: string, projectPath: string, filePath?: string) => {
    await postOpenInIde(ideId, projectPath, filePath)
  }, [])

  return { availableIdes, preferredIde, setPreferredIde, openProject, openFile, openWithIde }
}
