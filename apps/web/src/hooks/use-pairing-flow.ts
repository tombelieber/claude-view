import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { supabase } from '../lib/supabase'

/**
 * State machine for the pairing flow:
 *
 *   idle ──start()──▶ creating ──(offer response)──▶ showing-qr
 *     ▲                                                  │
 *     │                                        (device paired)
 *     │                                                  ▼
 *     └────reset()─── success | expired | error ─────────┘
 */
export type PairingState =
  | { kind: 'idle' }
  | { kind: 'creating' }
  | { kind: 'showing-qr'; token: string; expiresAt: Date; relayWsUrl: string }
  | { kind: 'success'; deviceId: string }
  | { kind: 'expired' }
  | { kind: 'error'; message: string; code?: string }

interface PairOfferResponse {
  token: string
  relay_ws_url: string
  expires_at: string
}

interface UsePairingFlowOptions {
  /**
   * device_id the web UI should pass to the edge function as
   * `issuing_device_id`. Since the web is an ephemeral client without a
   * registered device row, we send a synthetic marker that the edge function
   * accepts for web-initiated offers.
   */
  issuingDeviceId?: string
}

const DEFAULT_ISSUING_DEVICE_ID = 'web-issuer'

/**
 * use-pairing-flow — drive the device pairing state machine.
 *
 * - `start()`     — calls `pair-offer`, transitions to `showing-qr` with the QR token.
 * - Auto-expires  — `showing-qr` → `expired` when `expiresAt` passes.
 * - `reset()`     — return to `idle`.
 *
 * The success transition is expected to be driven by the devices Realtime
 * subscription in `use-devices.ts`. Consumers that want a reactive success
 * state can pass `onDevicePaired(deviceId)` after observing a new row.
 */
export function usePairingFlow(options: UsePairingFlowOptions = {}) {
  const { issuingDeviceId = DEFAULT_ISSUING_DEVICE_ID } = options
  const queryClient = useQueryClient()
  const [state, setState] = useState<PairingState>({ kind: 'idle' })
  const expiryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const clearExpiryTimer = useCallback(() => {
    if (expiryTimerRef.current) {
      clearTimeout(expiryTimerRef.current)
      expiryTimerRef.current = null
    }
  }, [])

  const reset = useCallback(() => {
    clearExpiryTimer()
    setState({ kind: 'idle' })
  }, [clearExpiryTimer])

  const start = useCallback(async () => {
    if (!supabase) {
      setState({
        kind: 'error',
        message: 'Account service unavailable. Sign in to pair a device.',
        code: 'SUPABASE_UNREACHABLE',
      })
      return
    }
    clearExpiryTimer()
    setState({ kind: 'creating' })
    try {
      const { data, error } = await supabase.functions.invoke<PairOfferResponse>('pair-offer', {
        body: { issuing_device_id: issuingDeviceId },
      })
      if (error) {
        setState({
          kind: 'error',
          message: error.message ?? 'Failed to create pairing code',
        })
        return
      }
      if (!data) {
        setState({ kind: 'error', message: 'Empty response from pair-offer' })
        return
      }
      const expiresAt = new Date(data.expires_at)
      setState({
        kind: 'showing-qr',
        token: data.token,
        expiresAt,
        relayWsUrl: data.relay_ws_url,
      })
      // Invalidate device list so any auto-created issuing row (e.g. first pair) shows up.
      queryClient.invalidateQueries({ queryKey: ['devices'] })

      // Schedule the auto-expire transition.
      const delay = Math.max(0, expiresAt.getTime() - Date.now())
      expiryTimerRef.current = setTimeout(() => {
        setState((current) => (current.kind === 'showing-qr' ? { kind: 'expired' } : current))
      }, delay)
    } catch (e) {
      setState({
        kind: 'error',
        message: e instanceof Error ? e.message : 'Unknown error',
      })
    }
  }, [issuingDeviceId, queryClient, clearExpiryTimer])

  /** Call when a new paired device has been observed — moves to `success`. */
  const markPaired = useCallback(
    (deviceId: string) => {
      clearExpiryTimer()
      setState((current) =>
        current.kind === 'showing-qr' ? { kind: 'success', deviceId } : current,
      )
    },
    [clearExpiryTimer],
  )

  useEffect(() => clearExpiryTimer, [clearExpiryTimer])

  return { state, start, reset, markPaired }
}
