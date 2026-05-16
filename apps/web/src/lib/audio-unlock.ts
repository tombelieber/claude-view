// apps/web/src/lib/audio-unlock.ts
//
// Owns the single shared Web Audio context and unlocks it on the first user
// gesture. Single responsibility: AudioContext lifecycle + autoplay unlock.
//
// WHY a module singleton (and not DI): an AudioContext is an inherently
// document-scoped scarce resource — browsers hard-cap ~6 per document and
// each one holds an audio HW thread. There is exactly one audio output for
// the page, so there is exactly one context. This is the same pattern Howler,
// use-sound, Slack and Discord's web clients use. It is an explicitly-imported
// service, not ambient global state reached into at random.
//
// WHY this exists: Chrome/Safari autoplay policy. An AudioContext created
// without prior user activation starts `suspended`; ctx.resume() invoked
// outside a user gesture (with no document sticky-activation) will not move it
// to `running`. Notification sounds fire exactly when the user is idle waiting
// for an agent — the precise no-activation condition — so without an explicit
// first-gesture unlock the oscillator plays into a silent suspended context
// 100% of the time. Resuming on the first pointerdown/keydown/touchstart
// guarantees the context is `running` long before any notification.

type UnlockListener = (running: boolean) => void

let ctx: AudioContext | null = null
let unlockArmed = false
const listeners = new Set<UnlockListener>()

const GESTURE_EVENTS = ['pointerdown', 'keydown', 'touchstart'] as const

function audioContextCtor(): typeof AudioContext | null {
  if (typeof window === 'undefined') return null
  return (
    window.AudioContext ??
    (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext ??
    null
  )
}

/**
 * The shared AudioContext, created lazily. Returns null only if the runtime
 * has no Web Audio support (or non-browser env).
 */
export function getSharedAudioContext(): AudioContext | null {
  if (ctx) return ctx
  const Ctor = audioContextCtor()
  if (!Ctor) return null
  ctx = new Ctor()
  return ctx
}

/** True only when the shared context exists AND is actually running. */
export function isAudioRunning(): boolean {
  return ctx?.state === 'running'
}

function notify(): void {
  const running = isAudioRunning()
  for (const fn of listeners) fn(running)
}

async function handleFirstGesture(): Promise<void> {
  const context = getSharedAudioContext()
  if (!context) return
  try {
    if (context.state === 'suspended') {
      await context.resume()
    }
  } catch {
    // resume() can reject if the gesture was not trusted; the next real
    // gesture re-attempts because the listeners are still armed below.
    if (context.state !== 'running') return
  }
  if (context.state === 'running') {
    teardownGestureListeners()
    notify()
  }
}

function gestureHandler(): void {
  void handleFirstGesture()
}

function teardownGestureListeners(): void {
  if (!unlockArmed || typeof window === 'undefined') return
  for (const evt of GESTURE_EVENTS) {
    window.removeEventListener(evt, gestureHandler, true)
  }
  unlockArmed = false
}

/**
 * Arm a one-time global unlock. Idempotent — safe to call from every mount of
 * the notification hook (React strict-mode double-invoke included).
 *
 * @param onChange optional callback invoked when the context becomes running.
 * @returns cleanup that detaches the passed listener (NOT the global arm —
 *          the unlock must outlive any single component).
 */
export function installAudioUnlock(onChange?: UnlockListener): () => void {
  if (onChange) {
    listeners.add(onChange)
    // Report current truth immediately so callers don't start out lying.
    onChange(isAudioRunning())
  }

  if (!unlockArmed && typeof window !== 'undefined' && !isAudioRunning()) {
    unlockArmed = true
    for (const evt of GESTURE_EVENTS) {
      window.addEventListener(evt, gestureHandler, {
        capture: true,
        passive: true,
      })
    }
  }

  return () => {
    if (onChange) listeners.delete(onChange)
  }
}

/**
 * Best-effort resume for explicit play paths. After the first gesture the
 * document has sticky activation, so this succeeds even when called from a
 * non-gesture stack (e.g. the session-transition effect). Returns whether the
 * context is running afterwards.
 */
export async function resumeSharedAudio(): Promise<boolean> {
  const context = getSharedAudioContext()
  if (!context) return false
  if (context.state === 'suspended') {
    try {
      await context.resume()
    } catch {
      // Still locked — a future gesture will unlock via the armed listeners.
    }
  }
  const running = context.state === 'running'
  if (running) notify()
  return running
}

/** Test-only: reset module singletons between cases. */
export function __resetAudioUnlockForTests(): void {
  teardownGestureListeners()
  ctx = null
  unlockArmed = false
  listeners.clear()
}
