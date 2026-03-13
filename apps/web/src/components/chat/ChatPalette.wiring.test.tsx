import { describe, expect, it } from 'vitest'
// @ts-ignore -- Vite ?raw import (not recognized by tsc)
import chatInputBarSource from './ChatInputBar.tsx?raw'

describe('ChatPalette wiring regression', () => {
  it('ChatInputBar imports ChatPalette from ./ChatPalette (not ../CommandPalette)', () => {
    expect(chatInputBarSource).toContain("from './ChatPalette'")
  })

  it('ChatInputBar accepts capabilities prop', () => {
    expect(chatInputBarSource).toContain('capabilities')
  })

  it('ChatInputBar accepts onModelSwitch prop', () => {
    expect(chatInputBarSource).toContain('onModelSwitch')
  })
})
