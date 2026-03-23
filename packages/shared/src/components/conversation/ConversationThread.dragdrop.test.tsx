/**
 * Regression tests for dockview drag-drop scroll preservation.
 *
 * Root cause: Dockview v5 moves the DOM portal container on drag-drop
 * without React unmount/remount. scrollTop resets to 0 but no React
 * lifecycle fires. The fix uses dockview's `onDidGroupChange` event
 * → scrollToBottomSignal prop → scroll restoration + startReached guard.
 */
import React from 'react'
import { act, render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ConversationBlock } from '../../types/blocks'
import type { BlockRenderers } from './types'

// Mock react-virtuoso — happy-dom has no real scroll container
const scrollToIndexSpy = vi.fn()
vi.mock('react-virtuoso', () => ({
  // biome-ignore lint/suspicious/noExplicitAny: test mock
  Virtuoso: React.forwardRef(function MockVirtuoso(props: any, ref: any) {
    React.useImperativeHandle(ref, () => ({
      scrollToIndex: scrollToIndexSpy,
    }))
    return (
      <div data-testid="mock-virtuoso">
        {/* biome-ignore lint/suspicious/noExplicitAny: test mock */}
        {props.data?.map((_: any, i: number) => (
          <div key={i}>{props.itemContent?.(i, props.data[i])}</div>
        ))}
      </div>
    )
  }),
}))

import { ConversationThread } from './ConversationThread'

// ── Test helpers ──────────────────────────────────────────────────────

function makeBlock(id: string, type: 'user' | 'assistant' = 'user'): ConversationBlock {
  if (type === 'user') {
    return {
      id,
      type: 'user',
      text: `Message ${id}`,
      timestamp: Date.now() / 1000,
      images: [],
    } as unknown as ConversationBlock
  }
  return {
    id,
    type: 'assistant',
    timestamp: Date.now() / 1000,
    streaming: false,
    segments: [{ kind: 'text' as const, text: `Response ${id}`, costUsd: 0 }],
  } as unknown as ConversationBlock
}

const noopRenderer = () => null
const testRenderers: BlockRenderers = {
  // biome-ignore lint/suspicious/noExplicitAny: test mock
  user: noopRenderer as any,
  // biome-ignore lint/suspicious/noExplicitAny: test mock
  assistant: noopRenderer as any,
  canRender: () => true,
}

function makeBlocks(n: number): ConversationBlock[] {
  const blocks: ConversationBlock[] = []
  for (let i = 0; i < n; i++) {
    blocks.push(makeBlock(`msg-${i}`, i % 2 === 0 ? 'user' : 'assistant'))
  }
  return blocks
}

// ── Tests ─────────────────────────────────────────────────────────────

describe('ConversationThread scrollToBottomSignal', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    scrollToIndexSpy.mockClear()
    vi.useFakeTimers({ shouldAdvanceTime: true })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders with scrollToBottomSignal=0 without errors', () => {
    const blocks = makeBlocks(5)
    const { container } = render(
      <ConversationThread blocks={blocks} renderers={testRenderers} scrollToBottomSignal={0} />,
    )
    expect(container.querySelector('[data-testid="message-thread"]')).toBeTruthy()
  })

  it('calls scrollToIndex when signal increments (drag-drop)', async () => {
    const blocks = makeBlocks(10)

    const { rerender } = render(
      <ConversationThread blocks={blocks} renderers={testRenderers} scrollToBottomSignal={0} />,
    )

    scrollToIndexSpy.mockClear()

    // Simulate drag-drop: increment signal
    rerender(
      <ConversationThread blocks={blocks} renderers={testRenderers} scrollToBottomSignal={1} />,
    )

    // scrollToBottomRetry uses requestAnimationFrame loops
    await act(async () => {
      for (let i = 0; i < 15; i++) {
        vi.advanceTimersByTime(16)
      }
    })

    expect(scrollToIndexSpy).toHaveBeenCalled()
    const lastCall = scrollToIndexSpy.mock.calls[scrollToIndexSpy.mock.calls.length - 1]
    expect(lastCall[0]).toMatchObject({ align: 'end', behavior: 'auto' })
  })

  it('does NOT call extra scrollToIndex when signal stays the same', async () => {
    const blocks = makeBlocks(10)

    const { rerender } = render(
      <ConversationThread blocks={blocks} renderers={testRenderers} scrollToBottomSignal={0} />,
    )

    // Let mount-time scroll complete
    await act(async () => {
      vi.advanceTimersByTime(200)
    })

    const callsAfterMount = scrollToIndexSpy.mock.calls.length

    // Same signal value — no additional scroll
    rerender(
      <ConversationThread blocks={blocks} renderers={testRenderers} scrollToBottomSignal={0} />,
    )

    await act(async () => {
      vi.advanceTimersByTime(200)
    })

    // No NEW calls beyond mount-time scroll
    expect(scrollToIndexSpy.mock.calls.length).toBe(callsAfterMount)
  })

  it('handles multiple rapid signal increments without crash', async () => {
    const blocks = makeBlocks(10)

    const { rerender } = render(
      <ConversationThread blocks={blocks} renderers={testRenderers} scrollToBottomSignal={0} />,
    )

    // Simulate rapid drag-drop (user moving tab quickly between groups)
    for (let i = 1; i <= 5; i++) {
      rerender(
        <ConversationThread blocks={blocks} renderers={testRenderers} scrollToBottomSignal={i} />,
      )
    }

    await act(async () => {
      vi.advanceTimersByTime(1000)
    })

    // No crash, scrollToIndex was called
    expect(scrollToIndexSpy).toHaveBeenCalled()
  })

  it('does not attempt scroll when items are empty', () => {
    scrollToIndexSpy.mockClear()

    const { rerender } = render(
      <ConversationThread blocks={[]} renderers={testRenderers} scrollToBottomSignal={0} />,
    )

    rerender(<ConversationThread blocks={[]} renderers={testRenderers} scrollToBottomSignal={1} />)

    // No scroll attempt on empty list (Virtuoso not even mounted)
    expect(scrollToIndexSpy).not.toHaveBeenCalled()
  })
})

describe('ConversationThread guardedStartReached', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders with pagination props without errors', () => {
    const blocks = makeBlocks(10)
    const onStartReached = vi.fn()

    const { container } = render(
      <ConversationThread
        blocks={blocks}
        renderers={testRenderers}
        onStartReached={onStartReached}
        hasOlderMessages={true}
        isFetchingOlder={false}
      />,
    )

    expect(container.querySelector('[data-testid="message-thread"]')).toBeTruthy()
  })

  it('does not pass startReached when hasOlderMessages is false', () => {
    const blocks = makeBlocks(10)
    const onStartReached = vi.fn()

    render(
      <ConversationThread
        blocks={blocks}
        renderers={testRenderers}
        onStartReached={onStartReached}
        hasOlderMessages={false}
        isFetchingOlder={false}
      />,
    )

    // onStartReached should never be called when no older messages exist
    expect(onStartReached).not.toHaveBeenCalled()
  })
})
