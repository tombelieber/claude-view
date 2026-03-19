import { describe, expect, it } from 'vitest'
import { getNestedField } from './get-nested-field.js'

describe('getNestedField', () => {
  it('gets simple key', () => {
    expect(getNestedField({ verdict: 'PASS' }, 'verdict')).toBe('PASS')
  })

  it('gets nested path', () => {
    expect(getNestedField({ result: { verdict: 'PASS' } }, 'result.verdict')).toBe('PASS')
  })

  it('returns undefined for missing key', () => {
    expect(getNestedField({ a: 1 }, 'b')).toBeUndefined()
  })

  it('returns undefined for null input', () => {
    expect(getNestedField(null, 'a')).toBeUndefined()
  })

  it('handles array index access', () => {
    expect(getNestedField({ items: ['a', 'b'] }, 'items.1')).toBe('b')
  })
})
