import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
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
      params={{ agentStateGroup: null, liveStatus: 'inactive', ...params }}
      tabLocation="header"
    />,
  )
}

describe('ChatTabRenderer', () => {
  it('shows green dot for autonomous live session (aligned with sidebar)', () => {
    const { container } = renderTab(undefined, {
      agentStateGroup: 'autonomous',
      liveStatus: 'cc_owned',
    })
    const dot = container.querySelector('span.bg-green-500')
    expect(dot).not.toBeNull()
  })

  it('shows amber dot for needs_you live session (aligned with sidebar)', () => {
    const { container } = renderTab(undefined, {
      agentStateGroup: 'needs_you',
      liveStatus: 'cc_owned',
    })
    const dot = container.querySelector('span.bg-amber-500')
    expect(dot).not.toBeNull()
  })

  it('shows gray dot when no live data', () => {
    const { container } = renderTab(undefined, {
      agentStateGroup: null,
      liveStatus: 'inactive',
    })
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
})
