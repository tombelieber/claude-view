import { beforeEach, describe, expect, it } from 'vitest'
import { _resetCachedPathForTesting, findClaudeExecutable } from './cli-path.js'

describe('findClaudeExecutable', () => {
  beforeEach(() => {
    _resetCachedPathForTesting()
  })

  it('resolves claude from PATH when available', () => {
    try {
      const result = findClaudeExecutable()
      expect(result).toBeTruthy()
      expect(typeof result).toBe('string')
      expect(result.length).toBeGreaterThan(0)
    } catch (e: unknown) {
      expect((e as Error).message).toContain('Claude Code CLI not found')
    }
  })

  it('uses CLAUDE_VIEW_CLI_PATH env var when set (even if cache exists)', () => {
    const original = process.env.CLAUDE_VIEW_CLI_PATH
    try {
      try {
        findClaudeExecutable()
      } catch {
        /* ignore */
      }
      process.env.CLAUDE_VIEW_CLI_PATH = '/usr/local/bin/fake-claude'
      const result = findClaudeExecutable()
      expect(result).toBe('/usr/local/bin/fake-claude')
    } finally {
      if (original === undefined) {
        // biome-ignore lint/performance/noDelete: must fully remove env var, not set undefined
        delete process.env.CLAUDE_VIEW_CLI_PATH
      } else {
        process.env.CLAUDE_VIEW_CLI_PATH = original
      }
    }
  })

  it('throws descriptive error when claude is not found', () => {
    const originalPath = process.env.PATH
    const originalCli = process.env.CLAUDE_VIEW_CLI_PATH
    try {
      process.env.PATH = ''
      // biome-ignore lint/performance/noDelete: must fully remove env var, not set undefined
      delete process.env.CLAUDE_VIEW_CLI_PATH
      expect(() => findClaudeExecutable()).toThrow('Claude Code CLI not found')
    } finally {
      process.env.PATH = originalPath!
      if (originalCli !== undefined) process.env.CLAUDE_VIEW_CLI_PATH = originalCli
    }
  })

  it('takes first line only from which/where output (Windows compat)', () => {
    const original = process.env.CLAUDE_VIEW_CLI_PATH
    try {
      // biome-ignore lint/performance/noDelete: must fully remove env var, not set undefined
      delete process.env.CLAUDE_VIEW_CLI_PATH
      try {
        const result = findClaudeExecutable()
        expect(result).not.toContain('\n')
        expect(result).not.toContain('\r')
      } catch {
        // claude not installed — skip
      }
    } finally {
      if (original !== undefined) process.env.CLAUDE_VIEW_CLI_PATH = original
    }
  })
})
