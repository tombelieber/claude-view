import { describe, it, expect } from 'vitest'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator } from './types'

import { buildActionItems } from './use-action-items'

describe('buildActionItems', () => {
  it('creates TurnSeparators for user and assistant', () => {
    const msgs: RichMessage[] = [
      { type: 'user', content: 'hello' },
      { type: 'assistant', content: 'hi there' },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(2)
    expect(items[0]).toMatchObject({ type: 'turn', role: 'user' })
    expect(items[1]).toMatchObject({ type: 'turn', role: 'assistant' })
  })

  it('creates ActionItem for thinking messages (not dropped)', () => {
    const msgs: RichMessage[] = [
      { type: 'thinking', content: 'pondering the meaning of life...' },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('system')
    expect(item.toolName).toBe('thinking')
    expect(item.label).toContain('pondering')
  })

  it('creates ActionItem for system messages', () => {
    const msgs: RichMessage[] = [
      { type: 'system', content: 'turn ended', category: 'system', metadata: { type: 'turn_duration', durationMs: 1500 } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('system')
    expect(item.toolName).toBe('turn_duration')
    expect(item.status).toBe('success')
  })

  it('creates ActionItem for system snapshot messages', () => {
    const msgs: RichMessage[] = [
      { type: 'system', content: '', category: 'snapshot', metadata: { type: 'file-history-snapshot' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('snapshot')
  })

  it('creates ActionItem for system queue messages', () => {
    const msgs: RichMessage[] = [
      { type: 'system', content: '', category: 'queue', metadata: { type: 'queue-operation', operation: 'enqueue' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('queue')
  })

  it('creates ActionItem for non-hook progress (agent_progress)', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', metadata: { type: 'agent_progress', agentId: 'a1', prompt: 'do stuff' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('agent')
    expect(item.toolName).toBe('agent_progress')
  })

  it('creates ActionItem for bash_progress', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', metadata: { type: 'bash_progress', command: 'ls -la' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('builtin')
    expect(item.label).toContain('ls -la')
  })

  it('creates ActionItem for mcp_progress', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', metadata: { type: 'mcp_progress', server: 'my-server', method: 'query' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('mcp')
  })

  it('creates ActionItem for hook_event progress (from SQLite via normalization)', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: 'Hook: PreToolUse — lint', category: 'hook', metadata: { type: 'hook_event', _hookEvent: {} } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('hook')
    expect(item.toolName).toBe('hook_event')
  })

  it('creates ActionItem for summary messages', () => {
    const msgs: RichMessage[] = [
      { type: 'summary', content: 'Session summary text', category: 'system', metadata: { summary: 'Session summary text', leafUuid: 'abc' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.toolName).toBe('summary')
    expect(item.label).toContain('Session summary')
  })

  it('still pairs tool_use + tool_result correctly', () => {
    const msgs: RichMessage[] = [
      { type: 'tool_use', content: '', name: 'Read', input: '{"file_path":"/foo"}', category: 'builtin' },
      { type: 'tool_result', content: 'file contents' },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.toolName).toBe('Read')
    expect(item.output).toBe('file contents')
    expect(item.status).toBe('success')
  })

  it('still handles hook_progress from JSONL', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', category: 'hook_progress', metadata: { type: 'hook_progress', hookEvent: 'PreToolUse', command: 'lint' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('hook_progress')
  })

  it('handles hook type messages (legacy live path, before normalization)', () => {
    const msgs: RichMessage[] = [
      { type: 'hook', content: 'lint check', name: 'PreToolUse', category: 'hook', ts: 100 },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('hook')
  })
})
