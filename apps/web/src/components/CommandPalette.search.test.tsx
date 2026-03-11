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
 * 3. SearchResults page uses unified API and searchEngine indicator
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

  it('SearchResults page uses searchEngine indicator from unified API', () => {
    // Positive assertion: SearchResults DOES reference the searchEngine field
    // from the unified API response (grep fallback indicator)
    expect(searchResultsSource).toContain('searchEngine')
    // Negative assertion: no direct grep hook usage
    expect(searchResultsSource).not.toContain("from '../hooks/use-grep'")
  })
})
