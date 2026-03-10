import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { RichMessage } from './RichPane'
import { UserMessage } from './RichPane'

describe('UserMessage', () => {
  it('shows "Queued" badge when pending is true', () => {
    const msg: RichMessage = {
      type: 'user',
      content: 'fix the bug',
      pending: true,
    }
    render(<UserMessage message={msg} />)
    expect(screen.getByText(/queued/i)).toBeInTheDocument()
  })

  it('does not show "Queued" badge for normal messages', () => {
    const msg: RichMessage = {
      type: 'user',
      content: 'fix the bug',
    }
    render(<UserMessage message={msg} />)
    expect(screen.queryByText(/queued/i)).not.toBeInTheDocument()
  })
})
