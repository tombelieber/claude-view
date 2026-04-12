import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ChatTabRenderer } from '../ChatTabRenderer'

// Create a mock api object matching IDockviewPanelHeaderProps.api
function mockApi(overrides?: Partial<{ title: string; isActive: boolean; close: () => void }>) {
  return {
    title: overrides?.title ?? 'Test Session',
    isActive: overrides?.isActive ?? false,
    close: overrides?.close ?? vi.fn(),
    id: 'panel-1',
    group: { id: 'group-1' },
  }
}

function mockContainerApi() {
  return {
    panels: [],
    groups: [],
  }
}

function renderTab(
  apiOverrides?: Partial<{ title: string; isActive: boolean; close: () => void }>,
  params?: Record<string, unknown>,
) {
  return render(
    <ChatTabRenderer
      // biome-ignore lint/suspicious/noExplicitAny: mock dockview API in test
      api={mockApi(apiOverrides) as any}
      // biome-ignore lint/suspicious/noExplicitAny: mock dockview API in test
      containerApi={mockContainerApi() as any}
      params={{ agentStateGroup: null, ownership: null, ...params }}
      tabLocation="header"
    />,
  )
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('ChatTabRenderer', () => {
  it('shows green dot for working session', () => {
    const { container } = renderTab(undefined, { status: 'working' })
    const dot = container.querySelector('span.bg-green-500')
    expect(dot).not.toBeNull()
  })

  it('shows amber dot for paused session', () => {
    const { container } = renderTab(undefined, { status: 'paused' })
    const dot = container.querySelector('span.bg-amber-500')
    expect(dot).not.toBeNull()
  })

  it('shows gray dot for done/no-data session', () => {
    const { container } = renderTab(undefined, { status: 'done' })
    const dot = container.querySelector('[class*="bg-gray-"]')
    expect(dot).not.toBeNull()
  })

  it('renders session title from api.title', () => {
    renderTab({ title: 'My Project' })
    expect(screen.getByText('My Project')).toBeDefined()
  })

  it('truncates long session titles to 120px', () => {
    const { container } = renderTab({
      title: 'A very long session title that should be truncated',
    })
    const titleSpan = container.querySelector('span.truncate')
    expect(titleSpan?.className).toContain('max-w-[120px]')
  })

  it('close button always visible on active tab', () => {
    const { container } = renderTab({ isActive: true })
    const closeBtn = container.querySelector('button')
    expect(closeBtn?.className).toContain('opacity-100')
  })

  it('close button always visible for tmux tab with kill tooltip', () => {
    const { container } = renderTab(
      { isActive: false },
      {
        ownership: { tmux: { cliSessionId: 'cv-abc123' } },
        tmuxSessionId: 'cv-abc123',
      },
    )
    const closeBtn = container.querySelector('button')
    expect(closeBtn?.className).toContain('opacity-100')
    expect(closeBtn?.title).toBe('Kill CLI session')
  })

  it('clicking close on tmux tab sends DELETE to kill session', () => {
    const mockFetch = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response())
    const closeFn = vi.fn()
    const { container } = renderTab(
      { close: closeFn },
      {
        ownership: { tmux: { cliSessionId: 'cv-abc123' } },
        tmuxSessionId: 'cv-abc123',
      },
    )
    fireEvent.click(container.querySelector('button')!)
    expect(mockFetch).toHaveBeenCalledWith('/api/cli-sessions/cv-abc123', { method: 'DELETE' })
    expect(closeFn).toHaveBeenCalled()
  })

  it('clicking close on non-tmux tab does not send DELETE', () => {
    const mockFetch = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response())
    const closeFn = vi.fn()
    const { container } = renderTab({ close: closeFn })
    fireEvent.click(container.querySelector('button')!)
    expect(mockFetch).not.toHaveBeenCalled()
    expect(closeFn).toHaveBeenCalled()
  })
})
