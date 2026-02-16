import { useEffect, useRef } from 'react'
import type { LiveSession } from './use-live-sessions'
import { useMonitorStore } from '../../store/monitor-store'

interface UseAutoFillOptions {
  sessions: LiveSession[]
  enabled: boolean
}

/** How long (ms) before an idle/done session becomes a swap candidate. */
const IDLE_SWAP_DELAY_MS = 5 * 60 * 1000 // 5 minutes

/** How long (ms) before an idle/done session gets a faded border style. */
const IDLE_FADE_DELAY_MS = 60 * 1000 // 60 seconds

/**
 * Auto-fill hook for the Monitor view.
 *
 * Behavior:
 * - When a new session appears, it is automatically shown if the grid has capacity.
 * - Hidden sessions are skipped.
 * - When a non-pinned session has been idle/done for 5+ minutes AND there are
 *   active sessions waiting for a slot, the idle session is swapped out (hidden).
 *
 * Returns a Set of session IDs that should display with a "faded" border
 * (idle for 60+ seconds, not pinned).
 */
export function useAutoFill(options: UseAutoFillOptions): Set<string> {
  const { sessions, enabled } = options
  const prevSessionIdsRef = useRef<Set<string>>(new Set())
  const idleTimersRef = useRef<Map<string, number>>(new Map())
  const fadedIdsRef = useRef<Set<string>>(new Set())

  // Track idle timestamps for sessions that transition to idle/done
  useEffect(() => {
    if (!enabled) return

    const now = Date.now()
    for (const session of sessions) {
      const isIdle = session.status === 'paused' || session.status === 'done'
      if (isIdle && !idleTimersRef.current.has(session.id)) {
        idleTimersRef.current.set(session.id, now)
      } else if (!isIdle) {
        // Session became active again -- clear its idle timestamp
        idleTimersRef.current.delete(session.id)
        fadedIdsRef.current.delete(session.id)
      }
    }

    // Clean up timers for sessions that no longer exist
    const currentIds = new Set(sessions.map((s) => s.id))
    for (const id of idleTimersRef.current.keys()) {
      if (!currentIds.has(id)) {
        idleTimersRef.current.delete(id)
        fadedIdsRef.current.delete(id)
      }
    }
  }, [sessions, enabled])

  // Auto-fill: add new sessions to the grid if capacity is available
  useEffect(() => {
    if (!enabled) return

    const store = useMonitorStore.getState()
    const currentIds = new Set(sessions.map((s) => s.id))
    const prevIds = prevSessionIdsRef.current

    // Find newly appeared sessions
    for (const id of currentIds) {
      if (prevIds.has(id)) continue // not new

      // Skip if already hidden by the user
      if (store.hiddenPaneIds.has(id)) continue

      // New session -- it's automatically visible (not in hiddenPaneIds),
      // so no explicit action needed. The grid renders all non-hidden sessions.
    }

    prevSessionIdsRef.current = currentIds
  }, [sessions, enabled])

  // Auto-swap: replace idle non-pinned sessions with waiting active sessions
  useEffect(() => {
    if (!enabled) return

    const interval = setInterval(() => {
      const store = useMonitorStore.getState()
      const now = Date.now()

      // Count currently visible sessions
      const visibleSessions = sessions.filter((s) => !store.hiddenPaneIds.has(s.id))

      // Update faded state (idle for 60+ seconds, not pinned)
      const nextFaded = new Set<string>()
      for (const session of visibleSessions) {
        const idleSince = idleTimersRef.current.get(session.id)
        if (
          idleSince &&
          now - idleSince >= IDLE_FADE_DELAY_MS &&
          !store.pinnedPaneIds.has(session.id)
        ) {
          nextFaded.add(session.id)
        }
      }
      fadedIdsRef.current = nextFaded

      // Only swap when using a fixed grid override (explicit capacity limit)
      if (!store.gridOverride) return

      // Find sessions waiting for a slot (hidden but active)
      const waitingSessions = sessions.filter(
        (s) =>
          store.hiddenPaneIds.has(s.id) &&
          s.status === 'working'
      )

      if (waitingSessions.length === 0) return

      // Find idle non-pinned visible sessions that have been idle for 5+ minutes
      const swapCandidates = visibleSessions.filter((s) => {
        if (store.pinnedPaneIds.has(s.id)) return false
        const idleSince = idleTimersRef.current.get(s.id)
        if (!idleSince) return false
        return now - idleSince >= IDLE_SWAP_DELAY_MS
      })

      // Swap: hide idle sessions, show waiting ones
      const swapCount = Math.min(swapCandidates.length, waitingSessions.length)
      for (let i = 0; i < swapCount; i++) {
        store.hidePane(swapCandidates[i].id)
        store.showPane(waitingSessions[i].id)
      }
    }, 10_000) // Check every 10 seconds

    return () => clearInterval(interval)
  }, [sessions, enabled])

  return fadedIdsRef.current
}

/**
 * CSS class to apply to panes that are in the "faded" state (idle 60+ seconds).
 * Use this with the Set returned by useAutoFill:
 *
 * ```tsx
 * const fadedIds = useAutoFill({ sessions, enabled: true })
 * <div className={cn('...', fadedIds.has(session.id) && FADED_PANE_CLASS)}>
 * ```
 */
export const FADED_PANE_CLASS = 'border-gray-800 opacity-60 transition-opacity duration-1000'
