import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ChatNoticeBlock } from '../NoticeBlock'
import type { NoticeBlock } from '../../../../../types/blocks'

function makeNotice(overrides: Partial<NoticeBlock>): NoticeBlock {
  return {
    type: 'notice',
    id: 'n-1',
    variant: 'error',
    data: null,
    ...overrides,
  } as NoticeBlock
}

// ── rate_limit variant ────────────────────────────────────────────────────────

describe('ChatNoticeBlock — rate_limit', () => {
  it('renders "Rate limited" for rejected status', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: {
            type: 'rate_limit',
            status: 'rejected',
          },
        })}
      />,
    )
    expect(screen.getByText(/Rate limited/)).toBeInTheDocument()
  })

  it('renders rateLimitType badge when present', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: {
            type: 'rate_limit',
            status: 'rejected',
            rateLimitType: 'tokens_per_minute',
          },
        })}
      />,
    )
    expect(screen.getByText('tokens_per_minute')).toBeInTheDocument()
  })

  it('renders utilization percentage when present', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: {
            type: 'rate_limit',
            status: 'rejected',
            utilization: 0.87,
          },
        })}
      />,
    )
    expect(screen.getByText('87%')).toBeInTheDocument()
  })

  it('renders overageStatus badge when present', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: {
            type: 'rate_limit',
            status: 'rejected',
            overageStatus: 'active',
          },
        })}
      />,
    )
    expect(screen.getByText('active')).toBeInTheDocument()
  })

  it('renders "Overage" text when isUsingOverage is true', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: {
            type: 'rate_limit',
            status: 'rejected',
            isUsingOverage: true,
          },
        })}
      />,
    )
    expect(screen.getByText('Overage')).toBeInTheDocument()
  })

  it('renders retry info when retryInMs present', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: { type: 'rate_limit', status: 'rejected' },
          retryInMs: 5000,
          retryAttempt: 2,
          maxRetries: 5,
        })}
      />,
    )
    expect(screen.getByText(/retry 2\/5/)).toBeInTheDocument()
  })

  it('renders nothing when status is "allowed"', () => {
    const { container } = render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: { type: 'rate_limit', status: 'allowed' },
        })}
      />,
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders warning style for allowed_warning status', () => {
    const { container } = render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'rate_limit',
          data: { type: 'rate_limit', status: 'allowed_warning' },
        })}
      />,
    )
    expect(screen.getByText(/Approaching rate limit/)).toBeInTheDocument()
    const div = container.querySelector('div')
    expect(div?.className).toContain('yellow')
  })
})

// ── context_compacted variant ─────────────────────────────────────────────────

describe('ChatNoticeBlock — context_compacted', () => {
  it('renders preTokens count', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'context_compacted',
          data: { type: 'context_compacted', trigger: 'auto', preTokens: 125000 },
        })}
      />,
    )
    expect(screen.getByText(/125,000 tokens/)).toBeInTheDocument()
  })

  it('renders "(manual)" suffix when trigger is manual', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'context_compacted',
          data: { type: 'context_compacted', trigger: 'manual', preTokens: 50000 },
        })}
      />,
    )
    expect(screen.getByText(/manual/)).toBeInTheDocument()
  })
})

// ── error variant ─────────────────────────────────────────────────────────────

describe('ChatNoticeBlock — error', () => {
  it('renders FATAL badge when fatal is true', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'error',
          data: { type: 'error', message: 'Critical failure', fatal: true },
        })}
      />,
    )
    expect(screen.getByText('FATAL')).toBeInTheDocument()
  })

  it('does not render FATAL badge when fatal is false', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'error',
          data: { type: 'error', message: 'Minor error', fatal: false },
        })}
      />,
    )
    expect(screen.queryByText('FATAL')).not.toBeInTheDocument()
  })

  it('renders the error message text', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'error',
          data: { type: 'error', message: 'Something failed here', fatal: false },
        })}
      />,
    )
    expect(screen.getByText('Something failed here')).toBeInTheDocument()
  })
})

// ── assistant_error variant ───────────────────────────────────────────────────

describe('ChatNoticeBlock — assistant_error', () => {
  it('renders messageId (first 12 chars)', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'assistant_error',
          data: {
            type: 'assistant_error',
            error: 'rate_limit',
            messageId: 'msg-abcdefghij-xyz',
          },
        })}
      />,
    )
    expect(screen.getByText('msg-abcdefgh')).toBeInTheDocument()
  })

  it('renders human-readable error label', () => {
    render(
      <ChatNoticeBlock
        block={makeNotice({
          variant: 'assistant_error',
          data: {
            type: 'assistant_error',
            error: 'billing_error',
            messageId: 'msg-1',
          },
        })}
      />,
    )
    expect(screen.getByText('Billing error')).toBeInTheDocument()
  })
})

// ── session_resumed variant ───────────────────────────────────────────────────

describe('ChatNoticeBlock — session_resumed', () => {
  it('renders "Resumed session" text', () => {
    render(<ChatNoticeBlock block={makeNotice({ variant: 'session_resumed', data: null })} />)
    expect(screen.getByText('Resumed session')).toBeInTheDocument()
  })
})

// ── unknown variant ───────────────────────────────────────────────────────────

describe('ChatNoticeBlock — unknown variant', () => {
  it('renders null for unknown variant', () => {
    const { container } = render(
      <ChatNoticeBlock
        // @ts-expect-error intentional unknown variant for resilience test
        block={makeNotice({ variant: 'totally_unknown', data: null })}
      />,
    )
    expect(container.firstChild).toBeNull()
  })
})
