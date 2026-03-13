import { describe, expect, it } from 'vitest'
import { COMMANDS } from './commands'
import { KNOWN_COMMAND_DESCRIPTIONS } from './palette-items'

describe('command palette backward compatibility', () => {
  const requiredCommands = [
    'clear',
    'compact',
    'help',
    'cost',
    'status',
    'commit',
    'test',
    'review',
    'debug',
    'deploy',
  ]

  it('all previously hardcoded well-known commands have descriptions in palette', () => {
    for (const cmd of requiredCommands) {
      expect(KNOWN_COMMAND_DESCRIPTIONS[cmd], `Missing description for /${cmd}`).toBeDefined()
    }
  })

  it('mode commands are handled by Permissions submenu, not slash commands', () => {
    const modes = ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions']
    for (const mode of modes) {
      expect(KNOWN_COMMAND_DESCRIPTIONS[mode]).toBeUndefined()
    }
  })

  it('original commands.ts COMMANDS array is unchanged (16 entries)', () => {
    expect(COMMANDS).toHaveLength(16)
  })
})
