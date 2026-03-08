import { describe, expect, it } from 'vitest'
import { countActivePromptFilters, defaultPromptFilters } from './use-prompt-filters'

describe('usePromptFilters', () => {
  it('counts zero active filters for defaults', () => {
    expect(countActivePromptFilters(defaultPromptFilters)).toBe(0)
  })

  it('counts intents as one filter group', () => {
    expect(countActivePromptFilters({ ...defaultPromptFilters, intents: ['fix', 'create'] })).toBe(
      1,
    )
  })

  it('counts non-any hasPaste toggle', () => {
    expect(countActivePromptFilters({ ...defaultPromptFilters, hasPaste: 'yes' })).toBe(1)
  })

  it('counts non-null complexity', () => {
    expect(countActivePromptFilters({ ...defaultPromptFilters, complexity: 'short' })).toBe(1)
  })

  it('counts non-null templateMatch', () => {
    expect(countActivePromptFilters({ ...defaultPromptFilters, templateMatch: 'template' })).toBe(1)
  })

  it('counts all filters combined', () => {
    expect(
      countActivePromptFilters({
        ...defaultPromptFilters,
        intents: ['fix'],
        branches: ['main'],
        models: ['claude-3-opus'],
        hasPaste: 'yes',
        complexity: 'detailed',
        templateMatch: 'unique',
      }),
    ).toBe(6)
  })
})
