import { describe, test, expect } from 'vitest'
import { getSessionTitle, cleanPreviewText } from './get-session-title'

describe('getSessionTitle', () => {
  test('returns preview when available', () => {
    expect(getSessionTitle('Fix auth bug')).toBe('Fix auth bug')
  })

  test('returns summary when preview is empty', () => {
    expect(getSessionTitle('', 'Fixed authentication')).toBe('Fixed authentication')
  })

  test('returns summary when preview is whitespace-only', () => {
    expect(getSessionTitle('  ', 'Some summary')).toBe('Some summary')
  })

  test('returns Untitled session when both empty', () => {
    expect(getSessionTitle('', null)).toBe('Untitled session')
  })

  test('returns Untitled session when both undefined', () => {
    expect(getSessionTitle(undefined, undefined)).toBe('Untitled session')
  })

  test('returns Untitled session when preview is empty and summary is empty string', () => {
    expect(getSessionTitle('', '')).toBe('Untitled session')
  })

  test('prefers preview over summary', () => {
    expect(getSessionTitle('Real title', 'Summary text')).toBe('Real title')
  })

  test('cleans preview text before checking (strips XML tags)', () => {
    expect(getSessionTitle('<tag>Hello</tag>', 'Fallback')).toBe('Hello')
  })

  test('cleans preview text before checking (strips quotes)', () => {
    expect(getSessionTitle('"Fix the bug"', 'Fallback')).toBe('Fix the bug')
  })

  test('cleans preview text before checking (collapses whitespace)', () => {
    expect(getSessionTitle('  hello   world  ', 'Fallback')).toBe('hello world')
  })

  test('falls back to summary when preview cleans to empty', () => {
    expect(getSessionTitle('<>', 'Good summary')).toBe('Good summary')
  })

  test('returns system prompt label for system prompt sessions', () => {
    expect(getSessionTitle('You are a helpful assistant', 'Summary')).toBe('System prompt session')
  })

  test('handles summary with whitespace-only gracefully', () => {
    expect(getSessionTitle('', '   ')).toBe('Untitled session')
  })
})

describe('cleanPreviewText', () => {
  test('unescapes \\n to space', () => {
    expect(cleanPreviewText('Fix bug\\nwith auth')).toBe('Fix bug with auth')
  })

  test('unescapes \\t to space', () => {
    expect(cleanPreviewText('col1\\tcol2')).toBe('col1 col2')
  })

  test('unescapes \\\\" to quote', () => {
    expect(cleanPreviewText('say \\"hello\\"')).toBe('say "hello"')
  })

  test('unescapes \\\\\\\\ to single backslash', () => {
    expect(cleanPreviewText('path\\\\to\\\\file')).toBe('path\\to\\file')
  })

  test('handles multiple escape sequences together', () => {
    expect(cleanPreviewText('line1\\nline2\\n\\tindented')).toBe('line1 line2 indented')
  })
})
