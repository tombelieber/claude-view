import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'

interface QrPayload {
  r: string // relay URL
  k: string // X25519 pubkey
  t: string // one-time token
  v: number
}

interface PairedDevice {
  device_id: string
  name: string
  paired_at: number
}

export function useQrCode(enabled: boolean) {
  return useQuery<QrPayload>({
    queryKey: ['pairing', 'qr'],
    queryFn: () => fetch('/api/pairing/qr').then((r) => r.json()),
    enabled,
    refetchInterval: 4 * 60 * 1000, // Refresh every 4 min (token expires at 5)
    staleTime: 0,
  })
}

export function usePairedDevices() {
  return useQuery<PairedDevice[]>({
    queryKey: ['pairing', 'devices'],
    queryFn: () => fetch('/api/pairing/devices').then((r) => r.json()),
  })
}

export function useUnpairDevice() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (deviceId: string) =>
      fetch(`/api/pairing/devices/${deviceId}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['pairing'] }),
  })
}
