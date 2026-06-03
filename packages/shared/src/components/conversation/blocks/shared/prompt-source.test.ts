import { describe, expect, it } from 'vitest'
import { promptSourceDisplay } from './prompt-source'

describe('promptSourceDisplay', () => {
  it('returns null when the field is absent', () => {
    expect(promptSourceDisplay(undefined)).toBeNull()
    expect(promptSourceDisplay(null)).toBeNull()
    expect(promptSourceDisplay('')).toBeNull()
  })

  it('hides the default human-typed origin from the chat view (signal over noise)', () => {
    const typed = promptSourceDisplay('typed')
    expect(typed).not.toBeNull()
    expect(typed?.chatVisible).toBe(false)
    expect(typed?.label).toBe('typed')
  })

  it('surfaces non-default origins (sdk, system) in the chat view', () => {
    expect(promptSourceDisplay('sdk')).toMatchObject({ label: 'SDK', chatVisible: true })
    expect(promptSourceDisplay('system')).toMatchObject({ label: 'system', chatVisible: true })
  })

  it('surfaces unknown future values verbatim rather than dropping them', () => {
    // Forward-compat: a new CLI prompt origin must not vanish (Zero Data Loss).
    const future = promptSourceDisplay('voice')
    expect(future).toMatchObject({ label: 'voice', chatVisible: true })
  })
})
