import { describe, expect, it } from 'vitest'
import type {
  AssistantBlock,
  ProgressBlock,
  SystemBlock,
  UserBlock,
} from '../../../../../types/blocks'
import { developerRegistry } from '../../developer/registry'
import { chatRegistry } from '../registry'

// ── Registry shape ────────────────────────────────────────────────────────────

describe('chatRegistry', () => {
  it('exports all required block type renderers', () => {
    expect(chatRegistry).toHaveProperty('user')
    expect(chatRegistry).toHaveProperty('assistant')
    expect(chatRegistry).toHaveProperty('interaction')
    expect(chatRegistry).toHaveProperty('turn_boundary')
    expect(chatRegistry).toHaveProperty('notice')
    expect(chatRegistry).toHaveProperty('system')
    expect(chatRegistry).toHaveProperty('progress')
    expect(chatRegistry).toHaveProperty('team_transcript')
  })
})

describe('developerRegistry', () => {
  // Developer mode is the zero-data-loss debug view — every block must render.
  // This test is a hard regression guard: if someone ever adds a canRender to
  // developerRegistry, this fails immediately.
  it('does NOT expose canRender (developer mode must render every block)', () => {
    expect(developerRegistry.canRender).toBeUndefined()
  })

  it('exports all required block type renderers', () => {
    expect(developerRegistry).toHaveProperty('user')
    expect(developerRegistry).toHaveProperty('assistant')
    expect(developerRegistry).toHaveProperty('interaction')
    expect(developerRegistry).toHaveProperty('turn_boundary')
    expect(developerRegistry).toHaveProperty('notice')
    expect(developerRegistry).toHaveProperty('system')
    expect(developerRegistry).toHaveProperty('progress')
    expect(developerRegistry).toHaveProperty('team_transcript')
  })
})

// ── chatRegistry.canRender — visibility rules ────────────────────────────────

describe('chatRegistry.canRender', () => {
  const canRender = chatRegistry.canRender!

  // ── Always-visible block types ──────────────────────────────────────────

  it('renders user blocks', () => {
    const block: UserBlock = { type: 'user', id: 'u1', text: 'hi', timestamp: 0 }
    expect(canRender(block)).toBe(true)
  })

  it('renders non-empty assistant blocks', () => {
    const block: AssistantBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [{ kind: 'text', text: 'hello' }],
      streaming: false,
    }
    expect(canRender(block)).toBe(true)
  })

  it('renders streaming assistant blocks even if segments are empty', () => {
    const block: AssistantBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [],
      streaming: true,
    }
    expect(canRender(block)).toBe(true)
  })

  // ── Empty assistant blocks filtered out ─────────────────────────────────

  it('filters out empty assistant blocks (no segments, no thinking, not streaming)', () => {
    const block: AssistantBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [],
      streaming: false,
    }
    expect(canRender(block)).toBe(false)
  })

  it('filters out assistant blocks with whitespace-only text segment', () => {
    const block: AssistantBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [{ kind: 'text', text: '   \n' }],
      streaming: false,
    }
    expect(canRender(block)).toBe(false)
  })

  // ── Hook noise filtered out ─────────────────────────────────────────────

  it('filters out progress blocks with variant=hook (REST channel hook)', () => {
    const block = {
      type: 'progress',
      id: 'p1',
      variant: 'hook',
      category: 'hook',
      ts: 0,
      parentToolUseId: null,
      data: { type: 'hook', hookEvent: 'PreToolUse', hookName: 'format-on-save' },
    } as unknown as ProgressBlock
    expect(canRender(block)).toBe(false)
  })

  it('still renders non-hook progress variants (bash, mcp, agent, etc.)', () => {
    const block = {
      type: 'progress',
      id: 'p1',
      variant: 'bash',
      category: 'builtin',
      ts: 0,
      parentToolUseId: null,
      data: { type: 'bash', output: 'hi', elapsedTimeSeconds: 1 },
    } as unknown as ProgressBlock
    expect(canRender(block)).toBe(true)
  })

  it('filters out system blocks with variant=hook_event (WebSocket channel hook)', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'hook_event',
      data: {
        type: 'hook_event',
        hookName: 'format-on-save',
        phase: 'PreToolUse',
        outcome: 'success',
      },
    } as unknown as SystemBlock
    expect(canRender(block)).toBe(false)
  })

  it('still renders non-hook system variants (session_init, queue_operation, etc.)', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'session_init',
      data: {} as SystemBlock['data'],
    } as SystemBlock
    expect(canRender(block)).toBe(true)
  })
})
