import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SessionSpinner } from './SessionSpinner'

interface MockMediaQueryList {
  matches: boolean
  media: string
  onchange: null
  addEventListener: ReturnType<typeof vi.fn>
  removeEventListener: ReturnType<typeof vi.fn>
  addListener: ReturnType<typeof vi.fn>
  removeListener: ReturnType<typeof vi.fn>
  dispatchEvent: ReturnType<typeof vi.fn>
}

function createMockMediaQueryList(matches: boolean): MockMediaQueryList {
  return {
    matches,
    media: '',
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }
}

function renderNeedsYouSpinner() {
  const now = Math.floor(Date.now() / 1000)
  return render(
    <SessionSpinner
      mode="live"
      model="claude-sonnet-4-20250514"
      durationSeconds={42}
      inputTokens={1200}
      outputTokens={240}
      agentStateGroup="needs_you"
      spinnerVerb="Working"
      lastActivityAt={now - 30}
    />
  )
}

describe('SessionSpinner', () => {
  beforeEach(() => {
    window.matchMedia = vi.fn(() => createMockMediaQueryList(false))
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('opens cache tooltip on hover in needs_you mode', async () => {
    const user = userEvent.setup()
    renderNeedsYouSpinner()

    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument()

    const trigger = screen.getByTitle(/Cache warm|Cache cold/i)
    await user.hover(trigger)

    const tooltip = screen.getByRole('tooltip')
    expect(tooltip).toBeInTheDocument()
    expect(tooltip.textContent).toMatch(/Cache Warm|Cache Cold/)
  })

  it('returns trigger to closed state after mouse leave', async () => {
    const user = userEvent.setup()
    renderNeedsYouSpinner()

    const trigger = screen.getByTitle(/Cache warm|Cache cold/i)
    await user.hover(trigger)

    await waitFor(() => {
      expect(trigger).not.toHaveAttribute('data-state', 'closed')
    })

    await user.unhover(trigger)

    await waitFor(() => {
      expect(trigger).toHaveAttribute('data-state', 'closed')
    })
  })
})
