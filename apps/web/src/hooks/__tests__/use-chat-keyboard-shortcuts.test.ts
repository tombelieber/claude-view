import { renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useChatKeyboardShortcuts } from '../use-chat-keyboard-shortcuts'

function mockDockviewApi(panels: { id: string; sessionId: string }[] = []) {
  const panelMocks = panels.map((p) => ({
    id: p.id,
    title: p.id,
    params: { sessionId: p.sessionId },
    api: {
      close: vi.fn(),
      setActive: vi.fn(),
      id: p.id,
    },
    group: {
      panels: [] as unknown[], // will be set below
    },
  }))

  // Wire up group.panels for each mock
  for (const p of panelMocks) {
    p.group.panels = panelMocks
  }

  return {
    panels: panelMocks,
    activePanel: panelMocks[0] ?? null,
    addPanel: vi.fn(),
  }
}

function fireKey(key: string, opts: Partial<KeyboardEvent> = {}) {
  const event = new KeyboardEvent('keydown', {
    key,
    ctrlKey: true,
    bubbles: true,
    ...opts,
  })
  document.dispatchEvent(event)
}

describe('useChatKeyboardShortcuts', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('does nothing when api is null', () => {
    // Should not throw
    renderHook(() => useChatKeyboardShortcuts(null))
    fireKey('t')
  })

  it('Ctrl+T adds a new panel', () => {
    const api = mockDockviewApi()
    // biome-ignore lint/suspicious/noExplicitAny: mock api
    renderHook(() => useChatKeyboardShortcuts(api as any))

    fireKey('t')

    expect(api.addPanel).toHaveBeenCalledTimes(1)
    expect(api.addPanel.mock.calls[0][0]).toMatchObject({
      component: 'chat',
      title: 'New Session',
    })
  })

  it('Ctrl+W closes active panel', () => {
    const api = mockDockviewApi([{ id: 'panel-1', sessionId: 'sess-1' }])
    // biome-ignore lint/suspicious/noExplicitAny: mock api
    renderHook(() => useChatKeyboardShortcuts(api as any))

    fireKey('w')

    expect(api.activePanel.api.close).toHaveBeenCalledTimes(1)
  })

  it('Ctrl+\\ splits active panel right', () => {
    const api = mockDockviewApi([{ id: 'panel-1', sessionId: 'sess-1' }])
    // biome-ignore lint/suspicious/noExplicitAny: mock api
    renderHook(() => useChatKeyboardShortcuts(api as any))

    fireKey('\\')

    expect(api.addPanel).toHaveBeenCalledTimes(1)
    const call = api.addPanel.mock.calls[0][0]
    expect(call.params.liveStatus).toBe('inactive')
    expect(call.position.direction).toBe('right')
  })

  it('Ctrl+Shift+\\ splits active panel down', () => {
    const api = mockDockviewApi([{ id: 'panel-1', sessionId: 'sess-1' }])
    // biome-ignore lint/suspicious/noExplicitAny: mock api
    renderHook(() => useChatKeyboardShortcuts(api as any))

    fireKey('\\', { shiftKey: true })

    expect(api.addPanel).toHaveBeenCalledTimes(1)
    const call = api.addPanel.mock.calls[0][0]
    expect(call.position.direction).toBe('below')
  })

  it('ignores shortcuts when target is input/textarea', () => {
    const api = mockDockviewApi()
    // biome-ignore lint/suspicious/noExplicitAny: mock api
    renderHook(() => useChatKeyboardShortcuts(api as any))

    // Create a fake input element target
    const input = document.createElement('input')
    document.body.appendChild(input)
    const event = new KeyboardEvent('keydown', {
      key: 't',
      ctrlKey: true,
      bubbles: true,
    })
    Object.defineProperty(event, 'target', { value: input })
    document.dispatchEvent(event)

    expect(api.addPanel).not.toHaveBeenCalled()
    document.body.removeChild(input)
  })
})
