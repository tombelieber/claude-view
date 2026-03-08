import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../lib/notify'
import type {
  MarketplaceActionRequest,
  MarketplaceInfo,
  PluginActionResponse,
} from '../types/generated'

async function fetchMarketplaces(): Promise<MarketplaceInfo[]> {
  const res = await fetch('/api/plugins/marketplaces')
  if (!res.ok) throw new Error('Failed to fetch marketplaces')
  return res.json()
}

export function useMarketplaces() {
  return useQuery({
    queryKey: ['marketplaces'],
    queryFn: fetchMarketplaces,
    staleTime: 60_000,
  })
}

async function runMarketplaceAction(req: MarketplaceActionRequest): Promise<PluginActionResponse> {
  const res = await fetch('/api/plugins/marketplaces/action', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `HTTP ${res.status}`)
  }
  const data: PluginActionResponse = await res.json()
  if (!data.success) throw new Error(data.message ?? 'Marketplace action failed')
  return data
}

export function useMarketplaceMutations() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: runMarketplaceAction,
    onSuccess: (_data, req) => {
      const verb = req.action === 'add' ? 'Added' : req.action === 'remove' ? 'Removed' : 'Updated'
      toast.success(`${verb} marketplace`, { duration: TOAST_DURATION.micro })
      queryClient.invalidateQueries({ queryKey: ['marketplaces'] })
      queryClient.invalidateQueries({ queryKey: ['plugins'] })
    },
    onError: (err, req) => {
      toast.error(`Failed to ${req.action} marketplace: ${err.message}`, {
        duration: TOAST_DURATION.extended,
      })
    },
  })
}
