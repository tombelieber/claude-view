import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { computeInitialPage } from './block-pagination'

export interface HistoryResult {
  type: 'HISTORY_OK'
  blocks: ConversationBlock[]
  total: number
  offset: number
}

/**
 * Probe the session for total block count, then fetch the tail page
 * using the shared computeInitialPage algorithm.
 */
export async function fetchInitialHistory(sessionId: string): Promise<HistoryResult> {
  const probeResponse = await fetch(
    `/api/sessions/${encodeURIComponent(sessionId)}/messages?limit=1&offset=0&format=block`,
  )
  if (!probeResponse.ok) throw new Error(`Failed to fetch history (${probeResponse.status})`)
  const probe = await probeResponse.json()

  const total = Number(probe.total) || 0
  if (total === 0) {
    return { type: 'HISTORY_OK', blocks: [], total: 0, offset: 0 }
  }

  const { offset: tailOffset, size: initialSize } = computeInitialPage(total)
  const params = new URLSearchParams({
    limit: String(initialSize),
    offset: String(tailOffset),
    format: 'block',
  })
  const dataResponse = await fetch(
    `/api/sessions/${encodeURIComponent(sessionId)}/messages?${params}`,
  )
  if (!dataResponse.ok) throw new Error(`Failed to fetch history (${dataResponse.status})`)
  const data = await dataResponse.json()

  return { type: 'HISTORY_OK', blocks: data.blocks ?? [], total, offset: tailOffset }
}
