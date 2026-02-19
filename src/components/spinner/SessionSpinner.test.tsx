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

/** The visible tooltip popper wrapper (not the hidden a11y span). */
function getPopperWrapper(): Element | null {
  return document.querySelector('[data-radix-popper-content-wrapper]')
}

describe('SessionSpinner', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    window.matchMedia = vi.fn(() => createMockMediaQueryList(false))
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('shows cache tooltip absent before hover, present after hover', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    renderNeedsYouSpinner()

    // Tooltip not visible before interaction
    expect(getPopperWrapper()).toBeNull()

    const trigger = screen.getByTitle(/Cache warm|Cache cold/i)
    await user.hover(trigger)

    await waitFor(() => {
      expect(getPopperWrapper()).not.toBeNull()
    })

    // Verify key tooltip sections
    const wrapper = getPopperWrapper()!
    expect(wrapper.textContent).toContain('Prompt Cache')
    expect(wrapper.textContent).toContain('Warm')
    expect(wrapper.textContent).toContain('5 min')
    expect(wrapper.textContent).toContain('90% cheaper')
    expect(wrapper.textContent).toContain('Learn about prompt caching')
  })

  it('renders countdown text and trigger with cache status title', () => {
    renderNeedsYouSpinner()

    const trigger = screen.getByTitle('Cache warm')
    expect(trigger).toBeInTheDocument()
    // The countdown text should show remaining time (4:30)
    expect(trigger.textContent).toMatch(/\d:\d{2}/)
  })
})
