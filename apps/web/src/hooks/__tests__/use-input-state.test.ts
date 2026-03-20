import { describe, expect, it } from 'vitest'
import { computeInputState } from '../use-input-state'

describe('computeInputState (FSM)', () => {
  it('own/active returns canSend true', () => {
    const state = computeInputState({ mode: 'own', subState: 'active' })
    expect(state.canSend).toBe(true)
    expect(state.disabled).toBe(false)
    expect(state.placeholder).toBe('Message Claude...')
  })

  it('own/streaming returns disabled with processing reason', () => {
    const state = computeInputState({ mode: 'own', subState: 'streaming' })
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Agent is processing...')
  })

  it('own/waiting_permission returns disabled with permission reason', () => {
    const state = computeInputState({ mode: 'own', subState: 'waiting_permission' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Waiting for your response above')
  })

  it('own/compacting returns disabled with compacting reason', () => {
    const state = computeInputState({ mode: 'own', subState: 'compacting' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Compacting context...')
  })

  it('connecting/initial returns disabled with starting reason', () => {
    const state = computeInputState({ mode: 'connecting', reason: 'initial' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Starting session...')
  })

  it('error/fatal returns disabled with error reason', () => {
    const state = computeInputState({ mode: 'error', reason: 'fatal' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session error')
  })

  it('history returns canSend true (all inactive sessions auto-resumable)', () => {
    const state = computeInputState({ mode: 'history' })
    expect(state.canSend).toBe(true)
    expect(state.disabled).toBe(false)
    expect(state.placeholder).toBe('Message Claude...')
  })

  it('blank returns disabled with empty reason', () => {
    const state = computeInputState({ mode: 'blank' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('')
  })

  it('watching returns disabled with controlled-elsewhere reason', () => {
    const state = computeInputState({ mode: 'watching' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session controlled elsewhere')
  })

  it('connecting/reconnecting returns disabled with reconnecting reason', () => {
    const state = computeInputState({ mode: 'connecting', reason: 'reconnecting' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Reconnecting...')
  })

  it('error/replaced returns disabled with session replaced reason', () => {
    const state = computeInputState({ mode: 'error', reason: 'replaced' })
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session replaced')
  })
})
