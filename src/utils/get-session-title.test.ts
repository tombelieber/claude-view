import { describe, test, expect } from 'vitest'
import { getSessionTitle } from './get-session-title'

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
