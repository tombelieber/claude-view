import { useState, useRef, useCallback, useEffect } from 'react'
import type { LiveSession } from '../components/live/use-live-sessions'

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

function playPreset(
  ctx: AudioContext,
  preset: SoundPreset,
  volume: number,
): void {
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
): UseNotificationSoundResult {
  const [settings, setSettings] =
    useState<NotificationSoundSettings>(loadSettings)
  const [audioUnlocked, setAudioUnlocked] = useState(false)

  // Lazy AudioContext — created on first play attempt
  const audioCtxRef = useRef<AudioContext | null>(null)

  // Map of sessionId → previous group for transition detection
  const prevGroupsRef = useRef<Map<string, string>>(new Map())

  // Debounce: timestamp of last played sound
  const lastPlayTimeRef = useRef(0)

  // -----------------------------------------------------------------------
  // AudioContext lifecycle
  // -----------------------------------------------------------------------

  const getAudioContext = useCallback((): AudioContext => {
    if (!audioCtxRef.current) {
      audioCtxRef.current = new AudioContext()
    }
    return audioCtxRef.current
  }, [])

  const ensureResumed = useCallback(
    async (ctx: AudioContext): Promise<void> => {
      if (ctx.state === 'suspended') {
        await ctx.resume()
      }
      setAudioUnlocked(true)
    },
    [],
  )

  // -----------------------------------------------------------------------
  // Play sound with debounce
  // -----------------------------------------------------------------------

  const playSound = useCallback(
    async (preset?: SoundPreset, vol?: number) => {
      const now = Date.now()
      if (now - lastPlayTimeRef.current < DEBOUNCE_COOLDOWN_MS) {
        return
      }
      lastPlayTimeRef.current = now

      const ctx = getAudioContext()
      await ensureResumed(ctx)
      playPreset(ctx, preset ?? settings.sound, vol ?? settings.volume)
    },
    [settings.sound, settings.volume, getAudioContext, ensureResumed],
  )

  // -----------------------------------------------------------------------
  // Settings persistence
  // -----------------------------------------------------------------------

  const updateSettings = useCallback(
    (patch: Partial<NotificationSoundSettings>) => {
      setSettings((prev) => {
        const next = { ...prev, ...patch }
        saveSettings(next)
        return next
      })
    },
    [],
  )

  // -----------------------------------------------------------------------
  // Preview — plays the current (or newly-set) preset, ignoring debounce
  // -----------------------------------------------------------------------

  const previewSound = useCallback(async () => {
    const ctx = getAudioContext()
    await ensureResumed(ctx)
    playPreset(ctx, settings.sound, settings.volume)
  }, [settings.sound, settings.volume, getAudioContext, ensureResumed])

  // -----------------------------------------------------------------------
  // Transition detection
  // -----------------------------------------------------------------------

  useEffect(() => {
    const prevGroups = prevGroupsRef.current
    let shouldDing = false

    for (const session of sessions) {
      const currentGroup = session.agentState.group
      const previousGroup = prevGroups.get(session.id)

      if (previousGroup !== undefined) {
        // Known session — check for transition into needs_you
        if (previousGroup !== 'needs_you' && currentGroup === 'needs_you') {
          shouldDing = true
        }
      }
      // If previousGroup is undefined, this is initial discovery — skip (no ding)
    }

    // Rebuild the map for the next comparison
    const nextGroups = new Map<string, string>()
    for (const session of sessions) {
      nextGroups.set(session.id, session.agentState.group)
    }
    prevGroupsRef.current = nextGroups

    // Trigger sound if any transition was detected and settings allow it
    if (shouldDing && settings.enabled) {
      playSound()
    }
  }, [sessions, settings.enabled, playSound])

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
