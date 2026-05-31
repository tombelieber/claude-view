import { useCallback, useEffect, useRef, useState } from 'react'
import type { LiveSession } from '../components/live/use-live-sessions'
import { getSharedAudioContext, installAudioUnlock, resumeSharedAudio } from '../lib/audio-unlock'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SoundPreset = 'ding' | 'chime' | 'bell'

export interface NotificationSoundSettings {
  enabled: boolean
  volume: number // 0.0 - 1.0
  sound: SoundPreset
}

export interface UseNotificationSoundResult {
  settings: NotificationSoundSettings
  updateSettings: (patch: Partial<NotificationSoundSettings>) => void
  previewSound: () => void
  audioUnlocked: boolean
}

export interface UseNotificationSoundOptions {
  initialized?: boolean
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'claude-view:notification-sound'

const DEFAULT_SETTINGS: NotificationSoundSettings = {
  enabled: true,
  volume: 0.7,
  sound: 'ding',
}

/** Minimum interval between notification sounds (ms). */
const DEBOUNCE_COOLDOWN_MS = 500

// ---------------------------------------------------------------------------
// localStorage helpers
// ---------------------------------------------------------------------------

function loadSettings(): NotificationSoundSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<NotificationSoundSettings>
      return { ...DEFAULT_SETTINGS, ...parsed }
    }
  } catch {
    // Corrupted storage — fall through to defaults
  }
  return { ...DEFAULT_SETTINGS }
}

function saveSettings(settings: NotificationSoundSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  } catch {
    // Storage full or unavailable — silently ignore
  }
}

// ---------------------------------------------------------------------------
// Sound engine — Web Audio API presets
// ---------------------------------------------------------------------------

function playPreset(ctx: AudioContext, preset: SoundPreset, volume: number): void {
  const now = ctx.currentTime

  switch (preset) {
    case 'ding': {
      const osc = ctx.createOscillator()
      const gain = ctx.createGain()
      osc.type = 'sine'
      osc.frequency.value = 800
      gain.gain.setValueAtTime(volume, now)
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.15)
      osc.connect(gain)
      gain.connect(ctx.destination)
      osc.start(now)
      osc.stop(now + 0.15)
      break
    }

    case 'chime': {
      // First tone: 600 Hz
      const osc1 = ctx.createOscillator()
      const gain1 = ctx.createGain()
      osc1.type = 'triangle'
      osc1.frequency.value = 600
      gain1.gain.setValueAtTime(volume, now)
      gain1.gain.exponentialRampToValueAtTime(0.001, now + 0.25)
      osc1.connect(gain1)
      gain1.connect(ctx.destination)
      osc1.start(now)
      osc1.stop(now + 0.25)

      // Second tone: 900 Hz at +100ms
      const osc2 = ctx.createOscillator()
      const gain2 = ctx.createGain()
      osc2.type = 'triangle'
      osc2.frequency.value = 900
      gain2.gain.setValueAtTime(volume, now + 0.1)
      gain2.gain.exponentialRampToValueAtTime(0.001, now + 0.25)
      osc2.connect(gain2)
      gain2.connect(ctx.destination)
      osc2.start(now + 0.1)
      osc2.stop(now + 0.25)
      break
    }

    case 'bell': {
      const osc = ctx.createOscillator()
      const gain = ctx.createGain()
      osc.type = 'sine'
      osc.frequency.value = 1200
      gain.gain.setValueAtTime(volume, now)
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.3)
      osc.connect(gain)
      gain.connect(ctx.destination)
      osc.start(now)
      osc.stop(now + 0.3)
      break
    }
  }
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useNotificationSound(
  sessions: LiveSession[],
  options: UseNotificationSoundOptions = {},
): UseNotificationSoundResult {
  const [settings, setSettings] = useState<NotificationSoundSettings>(loadSettings)
  const initialized = options.initialized ?? true

  // Truthful: only ever true when the shared AudioContext is actually
  // `running`. Driven by the global unlock module — never set optimistically.
  const [audioUnlocked, setAudioUnlocked] = useState(false)

  // Map of sessionId → previous group for transition detection
  const prevGroupsRef = useRef<Map<string, string>>(new Map())

  // Suppress the first live snapshot only. After initial load, a session that
  // first appears already in needs_you is a real new alert and should ding.
  const initialSnapshotSeenRef = useRef(false)

  // Debounce: timestamp of last played sound
  const lastPlayTimeRef = useRef(0)

  // Re-entrancy guard: playSound awaits an async resume(); without this, two
  // transitions landing inside that await window would both schedule a tone
  // (a doubled ding). One play in flight at a time — impossible state removed.
  const playInFlightRef = useRef(false)

  // -----------------------------------------------------------------------
  // AudioContext lifecycle — arm a one-time global gesture unlock so the
  // shared context is `running` before any notification fires (browser
  // autoplay policy). `audioUnlocked` mirrors the real ctx state.
  // -----------------------------------------------------------------------

  useEffect(() => installAudioUnlock(setAudioUnlocked), [])

  // -----------------------------------------------------------------------
  // Play sound with debounce
  // -----------------------------------------------------------------------

  const playSound = useCallback(
    async (preset?: SoundPreset, vol?: number) => {
      const now = Date.now()
      if (now - lastPlayTimeRef.current < DEBOUNCE_COOLDOWN_MS) {
        return
      }
      if (playInFlightRef.current) return
      playInFlightRef.current = true
      try {
        const ctx = getSharedAudioContext()
        if (!ctx) return
        // Best-effort resume. If still autoplay-locked, skip silently — the
        // armed global gesture listener will unlock for the next notification.
        // Do NOT schedule oscillators into a suspended context (the original
        // silent-failure bug) and do NOT consume the debounce slot on a no-op.
        const running = await resumeSharedAudio()
        if (!running) return

        lastPlayTimeRef.current = now
        playPreset(ctx, preset ?? settings.sound, vol ?? settings.volume)
      } finally {
        playInFlightRef.current = false
      }
    },
    [settings.sound, settings.volume],
  )

  // -----------------------------------------------------------------------
  // Settings persistence
  // -----------------------------------------------------------------------

  const updateSettings = useCallback((patch: Partial<NotificationSoundSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch }
      saveSettings(next)
      return next
    })
  }, [])

  // -----------------------------------------------------------------------
  // Preview — plays the current (or newly-set) preset, ignoring debounce
  // -----------------------------------------------------------------------

  const previewSound = useCallback(async () => {
    // Invoked from the Preview button click — itself a user gesture, so
    // resume() is guaranteed to succeed here.
    const ctx = getSharedAudioContext()
    if (!ctx) return
    const running = await resumeSharedAudio()
    if (!running) return
    playPreset(ctx, settings.sound, settings.volume)
  }, [settings.sound, settings.volume])

  // -----------------------------------------------------------------------
  // Transition detection
  // -----------------------------------------------------------------------

  useEffect(() => {
    const nextGroups = new Map<string, string>()
    for (const session of sessions) {
      nextGroups.set(session.id, session.agentState.group)
    }

    if (!initialized) {
      prevGroupsRef.current = nextGroups
      initialSnapshotSeenRef.current = false
      return
    }

    if (!initialSnapshotSeenRef.current) {
      prevGroupsRef.current = nextGroups
      initialSnapshotSeenRef.current = true
      return
    }

    const prevGroups = prevGroupsRef.current
    let shouldDing = false

    for (const session of sessions) {
      const currentGroup = session.agentState.group
      const previousGroup = prevGroups.get(session.id)

      if (currentGroup === 'needs_you' && previousGroup !== 'needs_you') {
        shouldDing = true
      }
    }

    prevGroupsRef.current = nextGroups

    // Trigger sound if any transition was detected and settings allow it
    if (shouldDing && settings.enabled) {
      playSound()
    }
  }, [sessions, settings.enabled, playSound, initialized])

  // -----------------------------------------------------------------------
  // Return
  // -----------------------------------------------------------------------

  return {
    settings,
    updateSettings,
    previewSound,
    audioUnlocked,
  }
}
