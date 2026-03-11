import { useQuery } from '@tanstack/react-query'
import type { FileDiffResponse } from '../types/generated/FileDiffResponse'
import type { FileHistoryResponse } from '../types/generated/FileHistoryResponse'

async function fetchFileHistory(sessionId: string): Promise<FileHistoryResponse> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/file-history`)
  if (!response.ok) {
    throw new Error(`Failed to fetch file history: ${response.status}`)
  }
  return response.json()
}

export function useFileHistory(sessionId: string | null, version?: number) {
  return useQuery({
    queryKey: ['file-history', sessionId, version ?? 0],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchFileHistory(sessionId)
    },
    enabled: !!sessionId,
  })
}

async function fetchFileDiff(
  sessionId: string,
  fileHash: string,
  from: number,
  to: number,
  filePath?: string,
): Promise<FileDiffResponse> {
  const params = new URLSearchParams({ from: String(from), to: String(to) })
  if (filePath) params.set('file_path', filePath)
  const response = await fetch(
    `/api/sessions/${encodeURIComponent(sessionId)}/file-history/${encodeURIComponent(fileHash)}/diff?${params}`,
  )
  if (!response.ok) {
    throw new Error(`Failed to fetch diff: ${response.status}`)
  }
  return response.json()
}

export function useFileDiff(
  sessionId: string | null,
  fileHash: string | null,
  from: number,
  to: number,
  filePath?: string,
) {
  return useQuery({
    queryKey: ['file-diff', sessionId, fileHash, from, to, filePath],
    queryFn: () => {
      if (!sessionId || !fileHash) throw new Error('sessionId and fileHash are required')
      return fetchFileDiff(sessionId, fileHash, from, to, filePath)
    },
    enabled: !!sessionId && !!fileHash && from >= 0 && to > 0,
  })
}
