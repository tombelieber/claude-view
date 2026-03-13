import { describe, expect, it } from 'vitest'
import { deriveInputBarState } from './control-status-map'

describe('deriveInputBarState', () => {
  // --- Regression: existing behavior preserved ---
  it('returns dormant when not live and not lazy-resumable', () => {
    expect(deriveInputBarState('idle', false)).toBe('dormant')
    expect(deriveInputBarState('idle', false, false)).toBe('dormant')
    expect(deriveInputBarState('idle', false, undefined)).toBe('dormant')
  })

  // --- New: lazy-resumable state ---
  it('returns active when canResumeLazy is true and isLive is false', () => {
    expect(deriveInputBarState('idle', false, true)).toBe('active')
  })

  // --- Existing behavior: isLive=true delegates to sessionState ---
  it('returns streaming for active session state when live', () => {
    expect(deriveInputBarState('active', true)).toBe('streaming')
  })

  it('returns active for waiting_input when live', () => {
    expect(deriveInputBarState('waiting_input', true)).toBe('active')
  })

  // --- canResumeLazy is ignored when isLive is true ---
  it('ignores canResumeLazy when isLive is true', () => {
    expect(deriveInputBarState('waiting_input', true, true)).toBe('active')
    expect(deriveInputBarState('active', true, true)).toBe('streaming')
  })
})
