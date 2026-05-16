import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  __resetAudioUnlockForTests,
  installAudioUnlock,
  isAudioRunning,
  resumeSharedAudio,
} from '../../lib/audio-unlock'
import { useNotificationSound } from '../use-notification-sound'

// ---------------------------------------------------------------------------
// Faithful AudioContext mock modelling the browser autoplay policy:
// a context starts `suspended`; resume() only transitions it to `running`
// when a user gesture has been observed (sticky activation). Without a
// gesture, resume()'s promise resolves but the state stays `suspended` —
// exactly Chrome/Safari behaviour, and exactly the reported bug.
// ---------------------------------------------------------------------------

let gestureGranted = false
const createOscillatorSpy = vi.fn()
const createGainSpy = vi.fn()

class MockAudioContext {
  state: 'suspended' | 'running' | 'closed' = 'suspended'
  currentTime = 0
  destination = {} as AudioDestinationNode

  async resume(): Promise<void> {
    // Promise resolves either way; state only flips when activated.
    if (gestureGranted) this.state = 'running'
  }

  createOscillator() {
    createOscillatorSpy()
    return {
      type: 'sine',
      frequency: { value: 0 },
      connect: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
    }
  }

  createGain() {
    createGainSpy()
    return {
      gain: {
        value: 0,
        setValueAtTime: vi.fn(),
        exponentialRampToValueAtTime: vi.fn(),
      },
      connect: vi.fn(),
    }
  }
}

type Sess = { id: string; agentState: { group: string } }
const sess = (id: string, group: 'autonomous' | 'needs_you'): Sess => ({
  id,
  agentState: { group },
})

// Dispatch a real user gesture on window. The autoplay gate opens *because*
// of the gesture, so grant activation immediately before dispatch.
async function fireGesture(): Promise<void> {
  gestureGranted = true
  await act(async () => {
    window.dispatchEvent(new Event('pointerdown'))
    await Promise.resolve()
    await Promise.resolve()
  })
}

beforeEach(() => {
  gestureGranted = false
  createOscillatorSpy.mockClear()
  createGainSpy.mockClear()
  localStorage.clear()
  ;(window as unknown as { AudioContext: unknown }).AudioContext = MockAudioContext
  ;(globalThis as unknown as { AudioContext: unknown }).AudioContext = MockAudioContext
  __resetAudioUnlockForTests()
})

afterEach(() => {
  __resetAudioUnlockForTests()
})

describe('audio-unlock module', () => {
  it('starts locked: context not running, resume() is a no-op without a gesture', async () => {
    expect(isAudioRunning()).toBe(false)
    const running = await resumeSharedAudio()
    expect(running).toBe(false)
    expect(isAudioRunning()).toBe(false)
  })

  it('unlocks the shared context on the first user gesture and notifies listeners', async () => {
    const onChange = vi.fn()
    installAudioUnlock(onChange)
    expect(onChange).toHaveBeenLastCalledWith(false) // truthful: locked

    await fireGesture()

    expect(isAudioRunning()).toBe(true)
    expect(onChange).toHaveBeenLastCalledWith(true) // truthful: now running
  })

  it('keydown and touchstart also unlock (not just pointerdown)', async () => {
    installAudioUnlock()
    gestureGranted = true
    await act(async () => {
      window.dispatchEvent(new Event('keydown'))
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(isAudioRunning()).toBe(true)
  })
})

describe('useNotificationSound — autoplay-policy regression', () => {
  it('REPRODUCES THE BUG: agent → needs_you with no prior gesture produces NO sound', async () => {
    const { rerender } = renderHook(({ sessions }) => useNotificationSound(sessions), {
      initialProps: { sessions: [sess('s1', 'autonomous')] as never[] },
    })

    // Agent stops and needs the user — but the user has not interacted with
    // the page (they were waiting). This is the exact reported scenario.
    await act(async () => {
      rerender({ sessions: [sess('s1', 'needs_you')] as never[] })
      await Promise.resolve()
      await Promise.resolve()
    })

    // Before the fix this silently scheduled an oscillator into a suspended
    // context (no audible output). The fix makes it skip cleanly instead of
    // pretending. Either way: no sound was produced for the user.
    expect(createOscillatorSpy).not.toHaveBeenCalled()
    expect(isAudioRunning()).toBe(false)
  })

  it('THE FIX: after one user gesture, agent → needs_you DOES ding', async () => {
    const { rerender } = renderHook(({ sessions }) => useNotificationSound(sessions), {
      initialProps: { sessions: [sess('s1', 'autonomous')] as never[] },
    })

    // User clicks somewhere once (anywhere on the page).
    await fireGesture()
    expect(isAudioRunning()).toBe(true)

    // Now the agent stops and needs them.
    await act(async () => {
      rerender({ sessions: [sess('s1', 'needs_you')] as never[] })
      await Promise.resolve()
      await Promise.resolve()
    })

    await vi.waitFor(() => {
      expect(createOscillatorSpy).toHaveBeenCalled()
    })
    expect(createGainSpy).toHaveBeenCalled()
  })

  it('audioUnlocked is truthful: false while suspended, true only when running', async () => {
    const { result } = renderHook(() => useNotificationSound([] as never[]))

    expect(result.current.audioUnlocked).toBe(false)

    await fireGesture()

    expect(result.current.audioUnlocked).toBe(true)
  })

  it('does not ding on initial discovery (session first seen already needs_you)', async () => {
    renderHook(() => useNotificationSound([sess('s1', 'needs_you')] as never[]))
    await fireGesture() // unlock so a ding *could* fire if logic were wrong

    await act(async () => {
      await Promise.resolve()
    })
    expect(createOscillatorSpy).not.toHaveBeenCalled()
  })

  it('does not ding when disabled, even after unlock + transition', async () => {
    localStorage.setItem(
      'claude-view:notification-sound',
      JSON.stringify({ enabled: false, volume: 0.7, sound: 'ding' }),
    )
    const { rerender } = renderHook(({ sessions }) => useNotificationSound(sessions), {
      initialProps: { sessions: [sess('s1', 'autonomous')] as never[] },
    })
    await fireGesture()

    await act(async () => {
      rerender({ sessions: [sess('s1', 'needs_you')] as never[] })
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(createOscillatorSpy).not.toHaveBeenCalled()
  })
})
