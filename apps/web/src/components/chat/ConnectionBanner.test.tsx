import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ConnectionBanner } from './ConnectionBanner'

describe('ConnectionBanner', () => {
  it('renders nothing when health is ok', () => {
    const { container } = render(<ConnectionBanner health="ok" />)
    expect(container.firstChild).toBeNull()
  })

  it('shows reconnecting message for degraded health', () => {
    render(<ConnectionBanner health="degraded" />)
    expect(screen.getByText('Reconnecting...')).toBeTruthy()
  })

  it('shows connection lost message for lost health', () => {
    render(<ConnectionBanner health="lost" />)
    expect(screen.getByText('Connection lost')).toBeTruthy()
  })

  it('shows retry button when onRetry provided and health is lost', () => {
    const onRetry = vi.fn()
    render(<ConnectionBanner health="lost" onRetry={onRetry} />)
    const button = screen.getByText('Retry')
    expect(button).toBeTruthy()
  })

  it('does not show retry button for degraded health', () => {
    render(<ConnectionBanner health="degraded" onRetry={vi.fn()} />)
    expect(screen.queryByText('Retry')).toBeNull()
  })

  it('calls onRetry when retry button is clicked', async () => {
    const onRetry = vi.fn()
    render(<ConnectionBanner health="lost" onRetry={onRetry} />)
    await userEvent.click(screen.getByText('Retry'))
    expect(onRetry).toHaveBeenCalledTimes(1)
  })
})
