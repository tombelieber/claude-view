/**
 * Regression test: ChatPanel listens to dockview onDidGroupChange
 * and passes scrollToBottomSignal to ChatSession.
 *
 * Root cause: Dockview v5 moves the DOM portal on drag-drop without
 * React remount. scrollTop resets to 0. onDidGroupChange is the signal.
 */
import { render, act } from '@testing-library/react'
import { describe, expect, it, vi, beforeEach } from 'vitest'

// Mock dependencies
vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}))

// Track what ChatSession receives
let capturedScrollSignal: number | undefined
vi.mock('../../pages/ChatSession', () => ({
  // biome-ignore lint/suspicious/noExplicitAny: test mock
  ChatSession: (props: any) => {
    capturedScrollSignal = props.scrollToBottomSignal
    return <div data-testid="chat-session" data-signal={props.scrollToBottomSignal} />
  },
}))

import { ChatPanel } from './ChatPanel'

// Mock dockview panel API
type GroupChangeCallback = () => void
function createMockPanelApi() {
  const groupChangeListeners: GroupChangeCallback[] = []
  const activeChangeListeners: Array<(e: { isActive: boolean }) => void> = []

  return {
    api: {
      isActive: true,
      onDidActiveChange: (cb: (e: { isActive: boolean }) => void) => {
        activeChangeListeners.push(cb)
        return { dispose: () => {} }
      },
      onDidGroupChange: (cb: GroupChangeCallback) => {
        groupChangeListeners.push(cb)
        return { dispose: vi.fn() }
      },
      updateParameters: vi.fn(),
      setTitle: vi.fn(),
    },
    fireGroupChange: () => {
      for (const cb of groupChangeListeners) cb()
    },
  }
}

describe('ChatPanel drag-drop signal', () => {
  beforeEach(() => {
    capturedScrollSignal = undefined
  })

  it('passes scrollToBottomSignal=0 on initial render', () => {
    const mock = createMockPanelApi()

    render(
      <ChatPanel
        params={{ sessionId: 'test-123', liveStatus: undefined, liveProjectPath: undefined }}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        api={mock.api as any}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        containerApi={{} as any}
      />,
    )

    expect(capturedScrollSignal).toBe(0)
  })

  it('increments scrollToBottomSignal on onDidGroupChange', () => {
    const mock = createMockPanelApi()

    render(
      <ChatPanel
        params={{ sessionId: 'test-123', liveStatus: undefined, liveProjectPath: undefined }}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        api={mock.api as any}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        containerApi={{} as any}
      />,
    )

    expect(capturedScrollSignal).toBe(0)

    // Simulate drag-drop: panel moved to new group
    act(() => {
      mock.fireGroupChange()
    })

    expect(capturedScrollSignal).toBe(1)
  })

  it('increments on each group change (multiple drag-drops)', () => {
    const mock = createMockPanelApi()

    render(
      <ChatPanel
        params={{ sessionId: 'test-123', liveStatus: undefined, liveProjectPath: undefined }}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        api={mock.api as any}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        containerApi={{} as any}
      />,
    )

    act(() => {
      mock.fireGroupChange()
    })
    act(() => {
      mock.fireGroupChange()
    })
    act(() => {
      mock.fireGroupChange()
    })

    expect(capturedScrollSignal).toBe(3)
  })

  it('disposes the onDidGroupChange listener on unmount', () => {
    const mock = createMockPanelApi()
    const disposeSpy = vi.fn()
    mock.api.onDidGroupChange = (_cb: GroupChangeCallback) => {
      return { dispose: disposeSpy }
    }

    const { unmount } = render(
      <ChatPanel
        params={{ sessionId: 'test-123', liveStatus: undefined, liveProjectPath: undefined }}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        api={mock.api as any}
        // biome-ignore lint/suspicious/noExplicitAny: test mock
        containerApi={{} as any}
      />,
    )

    unmount()
    expect(disposeSpy).toHaveBeenCalled()
  })
})
