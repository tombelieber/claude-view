import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, it } from 'vitest'
import { LiveMonitorEmptyState } from './LiveMonitorEmptyState'

function renderEmptyState(processCount: number) {
  return render(
    <MemoryRouter>
      <LiveMonitorEmptyState processCount={processCount} />
    </MemoryRouter>,
  )
}

describe('LiveMonitorEmptyState', () => {
  it('always offers a path into session history (fixes the first-run dead-end)', () => {
    renderEmptyState(0)
    const cta = screen.getByRole('link', { name: /browse your past sessions/i })
    expect(cta).toHaveAttribute('href', '/sessions')
  })

  it('guides the user to start a session when none are detected', () => {
    renderEmptyState(0)
    expect(screen.getByText(/no claude sessions running right now/i)).toBeInTheDocument()
    expect(screen.getByText(/watching for sessions/i)).toBeInTheDocument()
    // No false "processes detected" claim when there are none.
    expect(screen.queryByText(/process(es)? detected/i)).not.toBeInTheDocument()
  })

  it('surfaces detected processes with correct pluralization', () => {
    renderEmptyState(3)
    expect(screen.getByText(/3 Claude processes detected/i)).toBeInTheDocument()
    expect(screen.getByText(/report in via hooks/i)).toBeInTheDocument()
  })

  it('uses the singular noun for a single detected process', () => {
    renderEmptyState(1)
    expect(screen.getByText(/1 Claude process detected/i)).toBeInTheDocument()
    expect(screen.queryByText(/processes detected/i)).not.toBeInTheDocument()
  })
})
