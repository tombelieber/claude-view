// V1-hardening M1.4 — decoupled 1Hz ticker.
//
// Problem (before): `useLiveSessions` owned a `setInterval` that set
// React state every 1000 ms. That caused <App> to re-render every
// second, which rebuilt the <Outlet context={{…}}> object literal,
// which forced every page using `useOutletContext()` to re-render at
// 1 Hz even when no session data changed — leading to flicker, focus
// loss, and wasted battery.
//
// Fix: move `currentTime` into its own store. Components that need
// duration displays subscribe via `useTick()` and re-render 1 Hz.
// Components that do NOT display durations are untouched by the tick.
//
// Implementation: singleton interval with ref-counted subscribers.
// When any component mounts that calls `useTick()`, the interval
// starts. When the last unmounts, it stops.

import { useEffect, useSyncExternalStore } from 'react'

type Listener = () => void

let currentTime = Math.floor(Date.now() / 1000)
const listeners = new Set<Listener>()
let intervalId: ReturnType<typeof setInterval> | null = null

function startIntervalIfNeeded() {
  if (intervalId != null) return
  intervalId = setInterval(() => {
    currentTime = Math.floor(Date.now() / 1000)
    for (const listener of listeners) listener()
  }, 1000)
}

function stopIntervalIfIdle() {
  if (listeners.size === 0 && intervalId != null) {
    clearInterval(intervalId)
    intervalId = null
  }
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener)
  startIntervalIfNeeded()
  return () => {
    listeners.delete(listener)
    stopIntervalIfIdle()
  }
}

function getSnapshot(): number {
  return currentTime
}

function getServerSnapshot(): number {
  return currentTime
}

/**
 * Returns the current Unix timestamp in seconds, re-rendering the caller
 * once per second. Safe to call from many components simultaneously —
 * the interval is shared via subscriber ref-counting.
 *
 * Use this in components that render elapsed/remaining durations.
 * Do NOT use for components that just need a stable "now" at mount time
 * (use `useMemo(() => Date.now(), [])` for that).
 */
export function useTick(): number {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot)
}

/** Test-only: force the internal tick value (no interval running). */
export function __setTickForTest(seconds: number): void {
  currentTime = seconds
  for (const listener of listeners) listener()
}

/** Internal: called by the legacy useLiveSessions hook to supply a stable ref. */
export function __currentTickForLegacyConsumers(): number {
  return currentTime
}

/** Start the interval eagerly so the first consumer doesn't wait ~1s for first tick. */
export function __ensureTickRunning(): () => void {
  // Dummy subscription to keep interval running even when no components mounted yet.
  // Used by useLiveSessions so currentTime is fresh.
  const noop: Listener = () => {}
  listeners.add(noop)
  startIntervalIfNeeded()
  return () => {
    listeners.delete(noop)
    stopIntervalIfIdle()
  }
}

/** Hook variant of ensureTickRunning that runs once on mount. */
export function useEnsureTickRunning(): void {
  useEffect(() => __ensureTickRunning(), [])
}
