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
 * 3. SearchResults page uses per-session engines field for the grep indicator
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

  it('SearchResults page uses per-session engines field for grep indicator', () => {
    // Positive assertion: SearchResults checks per-session engines array,
    // not the removed response-level searchEngine field
    expect(searchResultsSource).toContain('engines')
    expect(searchResultsSource).toContain('hasGrepResults')
    // Negative assertion: response-level searchEngine is gone
    expect(searchResultsSource).not.toContain('searchEngine')
    // Negative assertion: no direct grep hook usage
    expect(searchResultsSource).not.toContain("from '../hooks/use-grep'")
  })
})
