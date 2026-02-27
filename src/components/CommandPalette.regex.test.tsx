import { describe, it, expect } from 'vitest'
import { hasRegexMetacharacters } from '../hooks/use-search'

describe('CommandPalette regex detection', () => {
  it('detects .* as regex', () => {
    expect(hasRegexMetacharacters('foo.*bar')).toBe(true)
  })

  it('detects \\b word boundary as regex', () => {
    expect(hasRegexMetacharacters('\\bword\\b')).toBe(true)
  })

  it('detects \\d digit class as regex', () => {
    expect(hasRegexMetacharacters('error\\d+')).toBe(true)
  })

  it('detects character class [a- as regex', () => {
    expect(hasRegexMetacharacters('[a-z]+')).toBe(true)
  })

  it('detects non-capturing group (?:  as regex', () => {
    expect(hasRegexMetacharacters('(?:foo|bar)')).toBe(true)
  })

  it('does not detect plain text as regex', () => {
    expect(hasRegexMetacharacters('hello world')).toBe(false)
  })

  it('does not detect project filter as regex', () => {
    expect(hasRegexMetacharacters('project:my-app')).toBe(false)
  })

  it('does not detect quoted phrase as regex', () => {
    expect(hasRegexMetacharacters('"exact phrase"')).toBe(false)
  })
})
