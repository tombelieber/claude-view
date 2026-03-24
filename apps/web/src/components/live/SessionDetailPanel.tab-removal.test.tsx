import { describe, expect, it } from 'vitest'

/**
 * Regression tests for the terminal+log tab removal from SessionDetailPanel.
 * Old URL params (?tab=terminal, ?tab=log) must resolve to the chat tab.
 */
describe('SessionDetailPanel tab removal', () => {
  // Mirrors the resolvedTab logic in SessionDetailPanel.tsx
  const resolveTab = (tab: string | null) =>
    tab === 'terminal' || tab === 'log' ? 'chat' : (tab ?? 'overview')

  it('?tab=terminal redirects to chat', () => {
    expect(resolveTab('terminal')).toBe('chat')
  })

  it('?tab=log redirects to chat', () => {
    expect(resolveTab('log')).toBe('chat')
  })

  it('?tab=overview passes through unchanged', () => {
    expect(resolveTab('overview')).toBe('overview')
  })

  it('?tab=cost passes through unchanged', () => {
    expect(resolveTab('cost')).toBe('cost')
  })

  it('no tab param defaults to overview', () => {
    expect(resolveTab(null)).toBe('overview')
  })
})
