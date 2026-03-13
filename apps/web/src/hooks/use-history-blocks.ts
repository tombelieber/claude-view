import { historyToBlocks } from '@claude-view/shared/lib'
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { useMemo } from 'react'
import { useSessionMessages } from './use-session-messages'

export interface HistoryBlocksResult {
  /** Conversation blocks from all loaded pages, ordered chronologically */
  blocks: ConversationBlock[]
  /** Whether older messages are available above */
  hasOlderMessages: boolean
  /** Call to load the next older chunk (for scroll-up trigger) */
  fetchOlderMessages: () => void
  /** True while any page is being fetched */
  isFetching: boolean
  /** True while fetching an older page specifically */
  isFetchingOlder: boolean
  /** True during the initial load (no data yet) */
  isLoading: boolean
  /** Total message count from backend */
  totalMessages: number
  /** Error from the query, if any */
  error: Error | null
}

export function useHistoryBlocks(sessionId: string | null): HistoryBlocksResult {
  const {
    data,
    error,
    hasPreviousPage,
    fetchPreviousPage,
    isFetchingPreviousPage,
    isFetching,
    isLoading,
  } = useSessionMessages(sessionId)

  const blocks = useMemo(() => {
    if (!data?.pages) return []
    // TanStack Query prepends pages via fetchPreviousPage — pages[0] is always the oldest.
    // Pages are already in chronological order (ascending offset). Sort is a safety net.
    const sortedPages = [...data.pages].sort((a, b) => a.offset - b.offset)
    const allMessages = sortedPages.flatMap((page) => page.messages)
    // Message (generated) is structurally compatible with HistoricalMessage — same field names.
    try {
      return historyToBlocks(allMessages)
    } catch (e) {
      console.error('[useHistoryBlocks] historyToBlocks threw on malformed messages:', e)
      return []
    }
  }, [data?.pages])

  // All pages from the same session share the same total — any page's .total is correct.
  const totalMessages = data?.pages?.[0]?.total ?? 0

  return {
    blocks,
    hasOlderMessages: hasPreviousPage ?? false,
    fetchOlderMessages: () => {
      if (hasPreviousPage && !isFetchingPreviousPage) {
        fetchPreviousPage()
      }
    },
    isFetching,
    isFetchingOlder: isFetchingPreviousPage,
    isLoading,
    totalMessages,
    error: error as Error | null,
  }
}
