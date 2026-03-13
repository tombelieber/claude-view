import { describe, expect, it } from 'vitest'
import { computeInputState } from './use-input-state'

describe('computeInputState', () => {
  // --- Regression: existing behavior ---
  it('returns canSend=false when not live and not lazy', () => {
    const state = computeInputState('idle', false)
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
  })

  // --- New: lazy-resumable allows sending ---
  it('returns canSend=true when canResumeLazy is true', () => {
    const state = computeInputState('idle', false, true)
    expect(state.canSend).toBe(true)
    expect(state.disabled).toBe(false)
    expect(state.placeholder).toBe('Message Claude...')
  })

  // --- Existing: live + waiting_input ---
  it('returns canSend=true for waiting_input when live', () => {
    const state = computeInputState('waiting_input', true)
    expect(state.canSend).toBe(true)
  })

  // --- Existing: live + active (streaming) ---
  it('returns canSend=false when agent is active', () => {
    const state = computeInputState('active', true)
    expect(state.canSend).toBe(false)
  })
})
