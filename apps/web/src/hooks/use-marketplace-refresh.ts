import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../lib/notify'
import type { RefreshStatusResponse } from '../types/generated'

async function fetchRefreshStatus(): Promise<RefreshStatusResponse> {
  const res = await fetch('/api/plugins/marketplaces/refresh-status')
  if (!res.ok) return { active: false, ops: {} }
  return res.json()
}

export function useMarketplaceRefresh() {
  const queryClient = useQueryClient()
  const prevActiveRef = useRef(false)

  const { data } = useQuery({
    queryKey: ['marketplace-refresh-status'],
    queryFn: fetchRefreshStatus,
    refetchInterval: (query) => {
      const d = query.state.data
      return d?.active ? 1000 : false
    },
  })

  const isActive = data?.active ?? false
  const statusMap = data?.ops ?? {}

  // Toast on batch completion (active → false transition)
  useEffect(() => {
    if (prevActiveRef.current && !isActive && data) {
      const ops = Object.values(data.ops)
      const succeeded = ops.filter((op) => op?.status === 'completed').length
      const failed = ops.filter((op) => op?.status === 'failed').length

      if (failed === 0 && succeeded > 0) {
        toast.success(`Updated ${succeeded} marketplace${succeeded !== 1 ? 's' : ''}`, {
          duration: TOAST_DURATION.micro,
        })
      } else if (failed > 0) {
        toast.warning(
          `Updated ${succeeded} marketplace${succeeded !== 1 ? 's' : ''}, ${failed} failed`,
          { duration: TOAST_DURATION.extended },
        )
      }

      queryClient.invalidateQueries({ queryKey: ['marketplaces'] })
      queryClient.invalidateQueries({ queryKey: ['plugins'] })
    }
    prevActiveRef.current = isActive
  }, [isActive, data, queryClient])

  const refreshAll = useCallback(
    async (names?: string[]) => {
      try {
        const body = names ? { names } : {}
        const res = await fetch('/api/plugins/marketplaces/refresh-all', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        })
        if (!res.ok) {
          const text = await res.text()
          throw new Error(text || `HTTP ${res.status}`)
        }
        // Start polling immediately
        queryClient.invalidateQueries({ queryKey: ['marketplace-refresh-status'] })
      } catch (err) {
        toast.error(
          `Failed to start marketplace refresh: ${err instanceof Error ? err.message : 'Unknown error'}`,
          { duration: TOAST_DURATION.extended },
        )
      }
    },
    [queryClient],
  )

  const getStatus = useCallback((name: string) => statusMap[name]?.status ?? null, [statusMap])

  const getError = useCallback((name: string) => statusMap[name]?.error ?? null, [statusMap])

  return {
    refreshAll,
    statusMap,
    isActive,
    getStatus,
    getError,
  }
}
