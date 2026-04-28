// apps/web/src/components/CommandPalette.search.test.tsx
import { describe, expect, it } from 'vitest'

// Import source files as raw strings via Vite's ?raw suffix.
// This avoids node:fs / @types/node and works in the happy-dom test environment.
import commandPaletteSource from './CommandPalette.tsx?raw'
import searchResultsSource from './SearchResults.tsx?raw'

/**
 * Regression tests for unified search wiring in CommandPalette.
 *
 * These tests verify that:
 * 1. There is no isRegex branching — all queries go through useSearch
 * 2. useGrep is NOT imported or used in the main search flow
 * 3. SearchResults page does NOT differentiate engines in the UI
 *    (grep is the only engine, so a "substring matches"
 *    badge would be noise — every result is a substring match)
 *
 * Design principle: "One Endpoint Per Capability" (CLAUDE.md)
 */

describe('CommandPalette search wiring', () => {
  it('does not import useGrep', () => {
    expect(commandPaletteSource).not.toContain("from '../hooks/use-grep'")
    expect(commandPaletteSource).not.toContain('useGrep')
  })

  it('does not use isRegex or hasRegexMetacharacters for routing', () => {
    expect(commandPaletteSource).not.toContain('isRegex')
    expect(commandPaletteSource).not.toContain('hasRegexMetacharacters')
  })

  it('SearchResults page does not differentiate engines in the UI', () => {
    // Negative: the old response-level engine flag is gone
    expect(searchResultsSource).not.toContain('searchEngine')
    // Negative: no per-session "came from grep" branching is rendered —
    // grep is the only engine, so the badge is noise.
    expect(searchResultsSource).not.toContain('hasGrepResults')
    expect(searchResultsSource).not.toContain('Substring matches')
    // Negative: no direct grep hook usage
    expect(searchResultsSource).not.toContain("from '../hooks/use-grep'")
  })
})
