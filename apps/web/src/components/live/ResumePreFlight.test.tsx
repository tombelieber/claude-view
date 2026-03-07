import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ResumePreFlight } from './ResumePreFlight'

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  })

  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  }
}

function estimatePayload(overrides: Record<string, unknown> = {}) {
  return {
    session_id: 'sess-1',
    history_tokens: 12000,
    cache_warm: true,
    first_message_cost: 0.0123,
    per_message_cost: 0.0021,
    has_pricing: true,
    model: 'claude-sonnet-4-20250514',
    explanation: 'ok',
    session_title: 'My session',
    project_name: '/repo',
    turn_count: 12,
    files_edited: 3,
    last_active_secs_ago: 120,
    ...overrides,
  }
}

describe('ResumePreFlight', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('only fetches estimate when open && sessionId', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(async () => {
      return new Response(JSON.stringify(estimatePayload()), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    })

    const wrapper = createWrapper()
    const { rerender } = render(
      <ResumePreFlight
        sessionId="sess-1"
        open={false}
        onOpenChange={() => {}}
        onResume={() => {}}
      />,
      { wrapper },
    )

    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(fetchSpy).not.toHaveBeenCalled()

    rerender(
      <ResumePreFlight
        sessionId="sess-1"
        open={true}
        onOpenChange={() => {}}
        onResume={() => {}}
      />,
    )

    await waitFor(() => {
      expect(fetchSpy).toHaveBeenCalled()
    })

    const firstCall = fetchSpy.mock.calls[0]
    expect(firstCall[0]).toBe('/api/control/estimate')
    expect(firstCall[1]).toMatchObject({ method: 'POST' })
  })

  it('shows unpriced warning when has_pricing is false', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () => {
      return new Response(
        JSON.stringify(
          estimatePayload({ has_pricing: false, first_message_cost: null, per_message_cost: null }),
        ),
        {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        },
      )
    })

    render(
      <ResumePreFlight
        sessionId="sess-1"
        open={true}
        onOpenChange={() => {}}
        onResume={() => {}}
      />,
      { wrapper: createWrapper() },
    )

    expect(
      await screen.findByText('Cost estimate unavailable for this model (pricing data missing).'),
    ).toBeInTheDocument()
  })

  it('calls onResume with sessionId directly on button click (no resume POST)', async () => {
    const onOpenChange = vi.fn()
    const onResume = vi.fn()
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      if (input === '/api/control/estimate') {
        return new Response(JSON.stringify(estimatePayload({ model: 'claude-opus-4-20250514' })), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      }
      return new Response('{}', { status: 404 })
    })

    render(
      <ResumePreFlight
        sessionId="sess-1"
        open={true}
        onOpenChange={onOpenChange}
        onResume={onResume}
      />,
      { wrapper: createWrapper() },
    )

    await screen.findByText('My session')
    const resumeButton = screen.getByRole('button', { name: 'Resume in Dashboard' })
    await waitFor(() => {
      expect(resumeButton).not.toBeDisabled()
    })
    fireEvent.click(resumeButton)

    // onResume called with ONLY sessionId (no controlId)
    expect(onResume).toHaveBeenCalledWith('sess-1')
    // Dialog closed
    expect(onOpenChange).toHaveBeenCalledWith(false)
    // NO fetch to /api/control/resume
    const fetchSpy = vi.mocked(globalThis.fetch)
    const resumeCalls = fetchSpy.mock.calls.filter((call) => call[0] === '/api/control/resume')
    expect(resumeCalls).toHaveLength(0)
  })
})
