import type { Device, RevokeReason } from '@claude-view/shared'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { supabase } from '../lib/supabase'

const DEVICES_KEY = ['devices'] as const

/**
 * List all devices for the current user.
 *
 * Uses PostgREST directly (RLS filters to `user_id = auth.uid()`). Subscribes
 * to Supabase Realtime for the `devices` table so any paired / revoked /
 * renamed event invalidates the cache immediately.
 *
 * If Supabase isn't configured (`supabase === null`), returns an empty list
 * without error — the UI should render a "Connect an account to pair" state.
 */
export function useDevices() {
  const queryClient = useQueryClient()

  const query = useQuery<Device[]>({
    queryKey: DEVICES_KEY,
    queryFn: async () => {
      if (!supabase) return []
      const { data, error } = await supabase
        .from('devices')
        .select('*')
        .order('last_seen_at', { ascending: false })
      if (error) throw new Error(error.message)
      return (data ?? []) as Device[]
    },
    staleTime: 30_000,
  })

  // Realtime subscription — invalidate the cache on any INSERT/UPDATE/DELETE.
  // One subscription per hook caller; cleaned up on unmount.
  useEffect(() => {
    if (!supabase) return
    const channel = supabase
      .channel('devices-changes')
      .on('postgres_changes', { event: '*', schema: 'public', table: 'devices' }, () => {
        queryClient.invalidateQueries({ queryKey: DEVICES_KEY })
      })
      .subscribe()

    return () => {
      void supabase!.removeChannel(channel)
    }
  }, [queryClient])

  return query
}

/**
 * Revoke a single device via the `devices-revoke` Edge Function.
 * Optimistically removes the device from the cached list; rolls back on error.
 */
export function useRevokeDevice() {
  const queryClient = useQueryClient()

  return useMutation<
    { device: Device },
    Error,
    { deviceId: string; reason?: RevokeReason },
    { previous?: Device[] }
  >({
    mutationFn: async ({ deviceId, reason = 'user_action' }) => {
      if (!supabase) throw new Error('Account service unavailable')
      const { data, error } = await supabase.functions.invoke<{ device: Device }>(
        'devices-revoke',
        { body: { device_id: deviceId, reason } },
      )
      if (error) throw new Error(error.message)
      if (!data) throw new Error('Empty response from devices-revoke')
      return data
    },
    onMutate: async ({ deviceId }) => {
      await queryClient.cancelQueries({ queryKey: DEVICES_KEY })
      const previous = queryClient.getQueryData<Device[]>(DEVICES_KEY)
      if (previous) {
        queryClient.setQueryData<Device[]>(
          DEVICES_KEY,
          previous.map((d) =>
            d.device_id === deviceId
              ? { ...d, revoked_at: new Date().toISOString(), revoked_reason: 'user_action' }
              : d,
          ),
        )
      }
      return { previous }
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(DEVICES_KEY, context.previous)
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: DEVICES_KEY })
    },
  })
}

/**
 * Revoke every device of the current user except the one whose device_id
 * is passed as `callingDeviceId`. When the web UI is the caller and the
 * Mac hasn't self-registered yet, pass an empty string to revoke ALL devices.
 */
export function useTerminateOtherDevices() {
  const queryClient = useQueryClient()

  return useMutation<{ revoked_count: number }, Error, { callingDeviceId: string }>({
    mutationFn: async ({ callingDeviceId }) => {
      if (!supabase) throw new Error('Account service unavailable')
      const { data, error } = await supabase.functions.invoke<{ revoked_count: number }>(
        'devices-terminate-others',
        { body: { calling_device_id: callingDeviceId } },
      )
      if (error) throw new Error(error.message)
      if (!data) throw new Error('Empty response from devices-terminate-others')
      return data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: DEVICES_KEY })
    },
  })
}
