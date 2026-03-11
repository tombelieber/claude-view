import { describe, expect, it } from 'vitest'
import { computeInputState } from '../use-input-state'

describe('computeInputState', () => {
  it('waiting_input + isLive returns canSend true', () => {
    const state = computeInputState('waiting_input', true)
    expect(state.canSend).toBe(true)
    expect(state.disabled).toBe(false)
    expect(state.placeholder).toBe('Message Claude...')
  })

  it('active returns disabled with processing reason', () => {
    const state = computeInputState('active', true)
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Agent is processing...')
  })

  it('waiting_permission returns disabled with permission reason', () => {
    const state = computeInputState('waiting_permission', true)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Waiting for your response above')
  })

  it('compacting returns disabled with compacting reason', () => {
    const state = computeInputState('compacting', true)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Compacting context...')
  })

  it('initializing returns disabled with starting reason', () => {
    const state = computeInputState('initializing', true)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Starting session...')
  })

  it('closed returns disabled with ended reason', () => {
    const state = computeInputState('closed', true)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session ended')
  })

  it('error returns disabled with error reason', () => {
    const state = computeInputState('error', true)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Session error')
  })

  it('isLive false overrides all states to disabled resume message', () => {
    const state = computeInputState('waiting_input', false)
    expect(state.canSend).toBe(false)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('Resume to send messages')
  })

  it('unknown state returns disabled with empty reason', () => {
    const state = computeInputState('something_unknown', true)
    expect(state.disabled).toBe(true)
    expect(state.disabledReason).toBe('')
  })
})
