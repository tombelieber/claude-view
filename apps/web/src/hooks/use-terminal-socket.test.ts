import { describe, expect, it } from 'vitest'
import type { UseTerminalSocketOptions } from './use-terminal-socket'

describe('useTerminalSocket', () => {
  it('accepts block mode in type union', () => {
    // Type-level assertion — if this compiles, the union includes 'block'
    const opts: UseTerminalSocketOptions = {
      sessionId: 'test',
      mode: 'block',
      scrollback: 0,
      enabled: false,
      onMessage: () => {},
    }
    expect(opts.mode).toBe('block')
  })
})
