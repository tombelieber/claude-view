import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ProgressBlock, SystemBlock } from '../../../../../types/blocks'
import { ChatProgressBlock } from '../ProgressBlock'
import { chatRegistry } from '../registry'
import { ChatSystemBlock } from '../SystemBlock'

// Regression tests for "hook noise in chat mode".
//
// Hooks reach the UI via TWO independent channels:
//   1. ProgressBlock(variant='hook')    — REST  /api/sessions/{id}/hook-events
//   2. SystemBlock (variant='hook_event') — WebSocket stream accumulator
//
// Both are filtered OUT by `chatRegistry.canRender` BEFORE ConversationThread
// wraps the item in its padding div — returning null from inside the renderer
// would leave a visible 12px gap in the list. Developer mode has no canRender,
// so these still render via DevProgressBlock / DevSystemBlock (zero data loss).
//
// The individual renderers (ChatProgressBlock / ChatSystemBlock) DO still
// render hook variants when called directly — they're defense-agnostic, and
// the suppression lives one layer up in the registry. Direct rendering is
// only exercised by Storybook stories and unit tests; production always goes
// through ConversationThread → canRender → renderer.

function makeHookProgress(): ProgressBlock {
  return {
    type: 'progress',
    id: 'p-hook-1',
    variant: 'hook',
    category: 'hook',
    ts: 1_700_000_000,
    parentToolUseId: null,
    data: {
      type: 'hook',
      hookEvent: 'PreToolUse',
      hookName: 'format-on-save',
      statusMessage: 'Running format-on-save',
    },
  } as unknown as ProgressBlock
}

function makeHookEventSystem(): SystemBlock {
  return {
    type: 'system',
    id: 's-hook-1',
    variant: 'hook_event',
    data: {
      type: 'hook_event',
      hookName: 'format-on-save',
      phase: 'PreToolUse',
      outcome: 'success',
    },
  } as unknown as SystemBlock
}

function makeBashProgress(): ProgressBlock {
  return {
    type: 'progress',
    id: 'p-bash-1',
    variant: 'bash',
    category: 'builtin',
    ts: 1_700_000_000,
    parentToolUseId: null,
    data: {
      type: 'bash',
      output: 'hello\n',
      elapsedTimeSeconds: 1.2,
    },
  } as unknown as ProgressBlock
}

function makeSessionInitSystem(): SystemBlock {
  return {
    type: 'system',
    id: 's-init-1',
    variant: 'session_init',
    data: {
      type: 'session_init',
      model: 'claude-sonnet-4-5',
      permissionMode: 'default',
      tools: [],
      mcpServers: [],
      slashCommands: [],
      claudeCodeVersion: '1.0',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: 'stream',
    },
  } as unknown as SystemBlock
}

// ── Primary contract: chatRegistry.canRender filters out hooks ──────────────

describe('chatRegistry.canRender — hook noise', () => {
  const canRender = chatRegistry.canRender!

  it('drops REST-channel hook progress blocks', () => {
    expect(canRender(makeHookProgress())).toBe(false)
  })

  it('drops WebSocket-channel hook_event system blocks', () => {
    expect(canRender(makeHookEventSystem())).toBe(false)
  })

  it('keeps non-hook progress blocks (bash)', () => {
    expect(canRender(makeBashProgress())).toBe(true)
  })

  it('keeps non-hook system blocks (session_init)', () => {
    expect(canRender(makeSessionInitSystem())).toBe(true)
  })
})

// ── Renderer-level sanity: direct rendering still works for Storybook ──────

describe('ChatProgressBlock — direct render (Storybook path)', () => {
  it('renders a hook progress block when called directly (not via canRender)', () => {
    const { container } = render(<ChatProgressBlock block={makeHookProgress()} />)
    // Production routes through canRender, but direct stories should still work.
    expect(container.firstChild).not.toBeNull()
  })

  it('renders a bash progress block', () => {
    const { container } = render(<ChatProgressBlock block={makeBashProgress()} />)
    expect(container.firstChild).not.toBeNull()
  })
})

describe('ChatSystemBlock — direct render (Storybook path)', () => {
  it('renders a hook_event system block when called directly', () => {
    const { container } = render(<ChatSystemBlock block={makeHookEventSystem()} />)
    expect(container.firstChild).not.toBeNull()
  })

  it('renders a session_init system block', () => {
    render(<ChatSystemBlock block={makeSessionInitSystem()} />)
    // SessionInitPill shows the model label.
    expect(screen.getByText(/claude-sonnet-4-5/)).toBeInTheDocument()
  })
})
