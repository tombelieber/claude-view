import { useEffect } from 'react'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../lib/notify'

/**
 * Fetches AI-detected session pattern alert on mount.
 * Shows as a Sonner toast (participates in queue, stacks properly).
 * Dismissed daily via localStorage.
 *
 * Uses both onDismiss (user-initiated) and onAutoClose (timer-expired)
 * to persist the dismiss — Sonner fires these as separate callbacks.
 */
export function usePatternAlert() {
  useEffect(() => {
    fetch('/api/facets/pattern-alert')
      .then((r) => {
        if (!r.ok) throw new Error(`pattern-alert: ${r.status}`)
        return r.json()
      })
      .then((data) => {
        if (!data.pattern) return
        const key = `pattern-alert-${data.pattern}-${new Date().toISOString().slice(0, 10)}`
        if (localStorage.getItem(key)) return

        const persistDismiss = () => localStorage.setItem(key, '1')

        toast(`${data.count} of your last sessions were "${data.pattern}"`, {
          description: data.tip,
          duration: TOAST_DURATION.persistent,
          onDismiss: persistDismiss,
          onAutoClose: persistDismiss,
        })
      })
      .catch((e) => console.error('pattern-alert fetch failed:', e))
  }, [])
}
