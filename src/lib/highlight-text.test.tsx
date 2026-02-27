import { describe, it, expect } from 'vitest'
import { render } from '@testing-library/react'
import { highlightText, escapeRegex } from './highlight-text'

describe('escapeRegex', () => {
  it('escapes special regex characters', () => {
    expect(escapeRegex('foo.*bar')).toBe('foo\\.\\*bar')
    expect(escapeRegex('[test]')).toBe('\\[test\\]')
    expect(escapeRegex('a+b?c')).toBe('a\\+b\\?c')
  })

  it('leaves normal characters unchanged', () => {
    expect(escapeRegex('hello world')).toBe('hello world')
  })
})

describe('highlightText', () => {
  it('returns plain text when query is empty', () => {
    const result = highlightText('Hello world', '')
    expect(result).toBe('Hello world')
  })

  it('returns plain text when query is whitespace only', () => {
    const result = highlightText('Hello world', '   ')
    expect(result).toBe('Hello world')
  })

  it('highlights matching text case-insensitively', () => {
    const result = highlightText('Hello World hello', 'hello')
    const { container } = render(<>{result}</>)
    const marks = container.querySelectorAll('mark')
    expect(marks).toHaveLength(2)
    expect(marks[0].textContent).toBe('Hello')
    expect(marks[1].textContent).toBe('hello')
  })

  it('preserves non-matching text', () => {
    const result = highlightText('Hello World', 'World')
    const { container } = render(<>{result}</>)
    expect(container.textContent).toBe('Hello World')
    const marks = container.querySelectorAll('mark')
    expect(marks).toHaveLength(1)
    expect(marks[0].textContent).toBe('World')
  })

  it('handles regex special characters in query safely', () => {
    const result = highlightText('foo.*bar', '.*')
    const { container } = render(<>{result}</>)
    const marks = container.querySelectorAll('mark')
    expect(marks).toHaveLength(1)
    expect(marks[0].textContent).toBe('.*')
  })

  it('returns original text when no match found', () => {
    const result = highlightText('Hello world', 'xyz')
    expect(result).toBe('Hello world')
  })

  it('applies correct CSS classes to mark elements', () => {
    const result = highlightText('Hello world', 'world')
    const { container } = render(<>{result}</>)
    const mark = container.querySelector('mark')
    expect(mark).not.toBeNull()
    expect(mark!.className).toContain('bg-amber-200')
  })
})
