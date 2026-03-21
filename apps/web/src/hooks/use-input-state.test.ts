import { describe, expect, it } from 'vitest'
import { computeInputState } from './use-input-state'

describe('computeInputState (FSM)', () => {
  // --- history mode ---
  it('history mode returns canSend=true', () => {
    const state = computeInputState({ mode: 'history' })
    expect(state.canSend).toBe(true)
    expect(state.disabled).toBe(false)
    expect(state.placeholder).toBe('Message Claude...')
  })

  // --- own/active (was: waiting_input, true) ---
  it('own/active returns canSend=true', () => {
    const state = computeInputState({ mode: 'own', subState: 'active' })
    expect(state.canSend).toBe(true)
    expect(state.disabled).toBe(false)
    expect(state.placeholder).toBe('Message Claude...')
  })

  // --- own/streaming (was: active, true) ---
  it('own/streaming returns disabled with processing reason', () => {
    const state = computeInputState({ mode: 'own', subState: 'streaming' })
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Agent is processing...')
  })

  // --- own/waiting_permission ---
  it('own/waiting_permission returns disabled with permission reason', () => {
    const state = computeInputState({ mode: 'own', subState: 'waiting_permission' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Waiting for your response above')
  })

  // --- own/compacting ---
  it('own/compacting returns disabled with compacting reason', () => {
    const state = computeInputState({ mode: 'own', subState: 'compacting' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Compacting context...')
  })

  // --- connecting/initial (was: initializing, true) ---
  it('connecting/initial returns disabled with starting reason', () => {
    const state = computeInputState({ mode: 'connecting', reason: 'initial' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Starting session...')
  })

  // --- error/fatal (was: error, true) ---
  it('error/fatal returns disabled with session error reason', () => {
    const state = computeInputState({ mode: 'error', reason: 'fatal' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session error')
  })

  // --- FSM-only states (no old equivalent) ---
  it('blank returns disabled with empty reason', () => {
    const state = computeInputState({ mode: 'blank' })
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('')
    expect(state.placeholder).toBe('')
  })

  it('watching returns disabled with controlled-elsewhere reason', () => {
    const state = computeInputState({ mode: 'watching' })
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session controlled elsewhere')
    expect(state.placeholder).toBe('')
  })

  it('connecting/reconnecting returns disabled with reconnecting reason', () => {
    const state = computeInputState({ mode: 'connecting', reason: 'reconnecting' })
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Reconnecting...')
    expect(state.placeholder).toBe('')
  })

  it('error/replaced returns disabled with session replaced reason', () => {
    const state = computeInputState({ mode: 'error', reason: 'replaced' })
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session replaced')
    expect(state.placeholder).toBe('')
  })
})
