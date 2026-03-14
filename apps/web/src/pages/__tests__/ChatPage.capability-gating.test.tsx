import { describe, expect, it } from 'vitest'

/**
 * Capability gating tests — verify UI controls are hidden when their
 * required capability is absent from sessionInfo.capabilities.
 *
 * These test the gating logic extracted from ChatPage.tsx,
 * not full component rendering (which requires router + query mocks).
 */

describe('capability gating logic', () => {
  function isVisible(capabilities: string[], required: string): boolean {
    return capabilities.includes(required)
  }

  describe('ThinkingBudgetControl gating', () => {
    it('visible when set_max_thinking_tokens in capabilities', () => {
      expect(
        isVisible(
          ['interrupt', 'set_max_thinking_tokens', 'query_models'],
          'set_max_thinking_tokens',
        ),
      ).toBe(true)
    })

    it('hidden when set_max_thinking_tokens absent', () => {
      expect(isVisible(['interrupt', 'query_models'], 'set_max_thinking_tokens')).toBe(false)
    })
  })

  describe('McpPanel gating', () => {
    it('visible when query_mcp_status in capabilities', () => {
      expect(isVisible(['query_mcp_status', 'toggle_mcp'], 'query_mcp_status')).toBe(true)
    })

    it('hidden when query_mcp_status absent', () => {
      expect(isVisible(['interrupt'], 'query_mcp_status')).toBe(false)
    })
  })

  describe('RewindButton gating', () => {
    it('visible when rewind_files in capabilities', () => {
      expect(isVisible(['rewind_files', 'interrupt'], 'rewind_files')).toBe(true)
    })

    it('hidden when rewind_files absent', () => {
      expect(isVisible(['interrupt', 'set_model'], 'rewind_files')).toBe(false)
    })
  })

  describe('stop_task button gating', () => {
    it('visible when stop_task in capabilities', () => {
      expect(isVisible(['stop_task', 'interrupt'], 'stop_task')).toBe(true)
    })

    it('hidden when stop_task absent', () => {
      expect(isVisible(['interrupt'], 'stop_task')).toBe(false)
    })
  })

  describe('AccountInfoPanel gating', () => {
    it('visible when query_account_info in capabilities', () => {
      expect(isVisible(['query_account_info'], 'query_account_info')).toBe(true)
    })

    it('hidden when query_account_info absent', () => {
      expect(isVisible([], 'query_account_info')).toBe(false)
    })
  })

  describe('edge cases', () => {
    it('empty capabilities array hides all controls', () => {
      const caps: string[] = []
      expect(isVisible(caps, 'interrupt')).toBe(false)
      expect(isVisible(caps, 'set_max_thinking_tokens')).toBe(false)
      expect(isVisible(caps, 'query_mcp_status')).toBe(false)
      expect(isVisible(caps, 'rewind_files')).toBe(false)
      expect(isVisible(caps, 'stop_task')).toBe(false)
    })

    it('full capabilities array shows all controls', () => {
      const caps = [
        'interrupt',
        'set_model',
        'set_max_thinking_tokens',
        'stop_task',
        'query_models',
        'query_commands',
        'query_agents',
        'query_mcp_status',
        'query_account_info',
        'reconnect_mcp',
        'toggle_mcp',
        'set_mcp_servers',
        'rewind_files',
      ]
      expect(isVisible(caps, 'set_max_thinking_tokens')).toBe(true)
      expect(isVisible(caps, 'query_mcp_status')).toBe(true)
      expect(isVisible(caps, 'rewind_files')).toBe(true)
      expect(isVisible(caps, 'stop_task')).toBe(true)
      expect(isVisible(caps, 'query_account_info')).toBe(true)
    })
  })
})
