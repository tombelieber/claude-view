import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { useCallback, useEffect, useRef, useState } from 'react'
import { computeInitialPage, computePreviousPage } from '../lib/block-pagination'

interface UseSubAgentBlocksOptions {
  sessionId: string
  agentId: string
  enabled: boolean
}

interface UseSubAgentBlocksResult {
  blocks: ConversationBlock[]
  isLoading: boolean
  isFetchingOlder: boolean
  hasOlderMessages: boolean
  fetchOlder: () => void
  error: string | null
}

/**
 * HTTP-based block loading for sub-agents with pagination.
 *
 * Same pattern as the main chat tab: probe for total → fetch tail page →
 * fetch older pages on scroll-up via computePreviousPage().
 */
export function useSubAgentBlocks({
  sessionId,
  agentId,
  enabled,
}: UseSubAgentBlocksOptions): UseSubAgentBlocksResult {
  const [blocks, setBlocks] = useState<ConversationBlock[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [isFetchingOlder, setIsFetchingOlder] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const offsetRef = useRef(0)
  const totalRef = useRef(0)
  const loadedRef = useRef(false)

  // Build the API URL for this sub-agent
  const baseUrl = `/api/sessions/${encodeURIComponent(sessionId)}/subagents/${encodeURIComponent(agentId)}/messages`

  // Initial load: probe total → fetch tail page
  useEffect(() => {
    if (!enabled || loadedRef.current) return
    loadedRef.current = true

    setIsLoading(true)
    setError(null)

    // Step 1: probe for total block count
    fetch(`${baseUrl}?limit=1&offset=0&format=block`)
      .then(async (r) => {
        if (!r.ok) {
          const body = await r.json().catch(() => ({}))
          throw new Error(body.error ?? `HTTP ${r.status}`)
        }
        return r.json()
      })
      .then((probe) => {
        const total = Number(probe.total) || 0
        totalRef.current = total

        if (total === 0) {
          setBlocks([])
          setIsLoading(false)
          return
        }

        // Step 2: fetch the tail page
        const { offset, size } = computeInitialPage(total)
        offsetRef.current = offset

        const params = new URLSearchParams({
          limit: String(size),
          offset: String(offset),
          format: 'block',
        })
        return fetch(`${baseUrl}?${params}`)
          .then(async (r) => {
            if (!r.ok) throw new Error(`HTTP ${r.status}`)
            return r.json()
          })
          .then((data) => {
            setBlocks(data.blocks ?? [])
            setIsLoading(false)
          })
      })
      .catch((err) => {
        setError(err.message)
        setIsLoading(false)
      })
  }, [enabled, baseUrl])

  // Fetch older blocks (triggered by ConversationThread's onStartReached)
  const fetchOlder = useCallback(() => {
    const prev = computePreviousPage(offsetRef.current)
    if (!prev || isFetchingOlder) return

    setIsFetchingOlder(true)

    const params = new URLSearchParams({
      limit: String(prev.limit),
      offset: String(prev.offset),
      format: 'block',
    })
    fetch(`${baseUrl}?${params}`)
      .then(async (r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`)
        return r.json()
      })
      .then((data) => {
        const older: ConversationBlock[] = data.blocks ?? []
        offsetRef.current = prev.offset
        setBlocks((current) => [...older, ...current])
        setIsFetchingOlder(false)
      })
      .catch(() => {
        setIsFetchingOlder(false)
      })
  }, [baseUrl, isFetchingOlder])

  const hasOlderMessages = offsetRef.current > 0

  return { blocks, isLoading, isFetchingOlder, hasOlderMessages, fetchOlder, error }
}
