import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MessageTyped, MAX_INDENT_LEVEL, INDENT_PX } from './MessageTyped'

/** Helper to create a minimal message for testing */
function makeMessage(overrides: Partial<{ role: string; content: string; timestamp: string }> = {}) {
  return {
    role: 'user' as const,
    content: 'Hello world',
    ...overrides,
  }
}

describe('ConversationThreading', () => {
  describe('indent rendering', () => {
    it('root messages have no indent (indent=0)', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={0} />
      )
      const article = container.querySelector('[role="article"]')!
      // No paddingLeft when indent is 0
      expect(article.getAttribute('style')).toBeNull()
    })

    it('child message has correct indent (indent=1)', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={1} />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article).toHaveStyle({ paddingLeft: `${INDENT_PX}px` })
    })

    it('grandchild has double indent (indent=2)', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={2} />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article).toHaveStyle({ paddingLeft: `${2 * INDENT_PX}px` })
    })

    it('maximum indent is capped at MAX_INDENT_LEVEL', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={10} />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article).toHaveStyle({ paddingLeft: `${MAX_INDENT_LEVEL * INDENT_PX}px` })
    })

    it('negative indent is clamped to 0', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={-3} />
      )
      const article = container.querySelector('[role="article"]')!
      // Should behave like indent=0, no paddingLeft
      expect(article.getAttribute('style')).toBeNull()
    })
  })

  describe('connector line for child messages', () => {
    it('child message renders dashed connector line', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={1} isChildMessage />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article).toHaveStyle({ borderLeftStyle: 'dashed' })
      expect(article).toHaveStyle({ borderLeftColor: '#9CA3AF' })
    })

    it('non-child message does not render dashed connector', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={0} isChildMessage={false} />
      )
      const article = container.querySelector('[role="article"]')!
      // Should not have dashed border style in inline styles
      const style = article.getAttribute('style')
      // style is null or does not contain 'dashed'
      expect(style === null || !style.includes('dashed')).toBe(true)
    })

    it('child message has thread-child CSS class', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={1} isChildMessage />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article.classList.contains('thread-child')).toBe(true)
    })
  })

  describe('ARIA attributes', () => {
    it('root message has aria-level=1', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={0} />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article.getAttribute('aria-level')).toBe('1')
    })

    it('child message has aria-level=2', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={1} />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article.getAttribute('aria-level')).toBe('2')
    })

    it('deeply nested message has capped aria-level', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} indent={10} />
      )
      const article = container.querySelector('[role="article"]')!
      // Capped at MAX_INDENT_LEVEL, so aria-level = MAX_INDENT_LEVEL + 1
      expect(article.getAttribute('aria-level')).toBe(String(MAX_INDENT_LEVEL + 1))
    })

    it('all messages have role="article"', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} />
      )
      const article = container.querySelector('[role="article"]')
      expect(article).not.toBeNull()
    })
  })

  describe('parentUuid prop', () => {
    it('sets data-parent-uuid attribute when parentUuid is provided', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} parentUuid="abc-123" indent={1} isChildMessage />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article.getAttribute('data-parent-uuid')).toBe('abc-123')
    })

    it('does not set data-parent-uuid when parentUuid is not provided', () => {
      const { container } = render(
        <MessageTyped message={makeMessage()} />
      )
      const article = container.querySelector('[role="article"]')!
      expect(article.hasAttribute('data-parent-uuid')).toBe(false)
    })
  })
})
