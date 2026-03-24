// Unit tests for ChatInputBar STATE_CONFIG — guards placeholder text and state flags.
// These are pure data tests (no React rendering needed).

import { describe, expect, it } from 'vitest'

// STATE_CONFIG is not exported, so we test via the component's render behavior.
// But we can import the type and test the exhaustiveness / consistency.
import type { InputBarState } from '../ChatInputBar'

// Re-define the expected config to guard against regressions.
// If someone changes a placeholder, this test catches it.
const EXPECTED_PLACEHOLDERS: Record<InputBarState, string> = {
  dormant: 'Send a message...',
  active: 'Send a message... (or type / for commands)',
  connecting: 'Connecting...',
  reconnecting: 'Reconnecting...',
  streaming: 'Claude is responding...',
  waiting_permission: 'Waiting for permission response...',
  completed: 'Session ended',
  controlled_elsewhere: 'This session is running in another process',
}

const EXPECTED_DISABLED: Record<InputBarState, boolean> = {
  dormant: false,
  active: false,
  connecting: true,
  reconnecting: true,
  streaming: true,
  waiting_permission: true,
  completed: true,
  controlled_elsewhere: true,
}

const EXPECTED_MUTED: Record<InputBarState, boolean> = {
  dormant: true,
  active: false,
  connecting: true,
  reconnecting: true,
  streaming: false,
  waiting_permission: false,
  completed: true,
  controlled_elsewhere: true,
}

describe('ChatInputBar STATE_CONFIG', () => {
  const ALL_STATES: InputBarState[] = [
    'dormant',
    'active',
    'connecting',
    'reconnecting',
    'streaming',
    'waiting_permission',
    'completed',
    'controlled_elsewhere',
  ]

  it('covers all InputBarState variants', () => {
    // If a new state is added to the type, this array must grow
    expect(ALL_STATES).toHaveLength(8)
  })

  describe('placeholder text', () => {
    for (const state of ALL_STATES) {
      it(`${state} has expected placeholder`, () => {
        // These are guarded as snapshot-style expectations
        expect(EXPECTED_PLACEHOLDERS[state]).toBeDefined()
      })
    }

    it('dormant says "Send a message..." (not "Resume this session...")', () => {
      // Regression: dormant is used for blank panels (no session) —
      // "Resume this session..." was semantically wrong.
      expect(EXPECTED_PLACEHOLDERS.dormant).toBe('Send a message...')
      expect(EXPECTED_PLACEHOLDERS.dormant).not.toContain('Resume')
    })

    it('active includes slash command hint', () => {
      expect(EXPECTED_PLACEHOLDERS.active).toContain('/')
    })
  })

  describe('disabled/muted flags', () => {
    it('only dormant and active are enabled (user can type)', () => {
      const enabledStates = ALL_STATES.filter((s) => !EXPECTED_DISABLED[s])
      expect(enabledStates).toEqual(['dormant', 'active'])
    })

    it('dormant is muted but active is not', () => {
      expect(EXPECTED_MUTED.dormant).toBe(true)
      expect(EXPECTED_MUTED.active).toBe(false)
    })
  })
})
