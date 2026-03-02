import { describe, expect, it } from 'vitest'
import { COMMANDS, filterCommands } from './commands'

describe('commands', () => {
  it('exports at least 10 commands', () => {
    expect(COMMANDS.length).toBeGreaterThanOrEqual(10)
  })
  it('every command has name, description, and category', () => {
    for (const cmd of COMMANDS) {
      expect(cmd.name).toBeTruthy()
      expect(cmd.description).toBeTruthy()
      expect(cmd.category).toBeTruthy()
    }
  })
  it('filters by prefix', () => {
    const results = filterCommands('com')
    expect(
      results.every((c) => c.name.includes('com') || c.description.toLowerCase().includes('com')),
    ).toBe(true)
  })
  it('returns all commands for empty query', () => {
    expect(filterCommands('')).toEqual(COMMANDS)
  })
  it('filters case-insensitively', () => {
    const upper = filterCommands('PLAN')
    const lower = filterCommands('plan')
    expect(upper).toEqual(lower)
    expect(upper.length).toBeGreaterThan(0)
  })
  it('returns empty array for no matches', () => {
    expect(filterCommands('zzzznotacommand')).toEqual([])
  })
  it('matches by description', () => {
    const results = filterCommands('git')
    expect(results.some((c) => c.name === 'commit')).toBe(true)
  })
  it('has valid categories', () => {
    const validCategories = new Set(['mode', 'session', 'action', 'info'])
    for (const cmd of COMMANDS) {
      expect(validCategories.has(cmd.category)).toBe(true)
    }
  })
})
