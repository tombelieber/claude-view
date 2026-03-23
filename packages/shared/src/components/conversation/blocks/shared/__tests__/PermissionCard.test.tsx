import type { PermissionRequest } from '../../../../../types/sidecar-protocol'
import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PermissionCard } from '../PermissionCard'

// --- Mock sonner toast ---
const mockToastError = vi.fn()
vi.mock('sonner', () => ({
  toast: {
    error: (msg: string, opts?: unknown) => mockToastError(msg, opts),
  },
}))

function makePermission(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    type: 'permission_request',
    requestId: 'req-1',
    toolName: 'Bash',
    toolInput: { command: 'echo hello' },
    toolUseID: 'tu-1',
    timeoutMs: 5000,
    ...overrides,
  }
}

beforeEach(() => {
  vi.useFakeTimers()
  mockToastError.mockClear()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('PermissionCard — hardening fixes', () => {
  it('shows toast when permission countdown reaches 0 (Fix 1)', () => {
    const onRespond = vi.fn()
    render(
      <PermissionCard permission={makePermission({ timeoutMs: 3000 })} onRespond={onRespond} />,
    )

    // Advance timer to 0 (3 seconds)
    act(() => {
      vi.advanceTimersByTime(3000)
    })

    // onRespond should have been called with denied
    expect(onRespond).toHaveBeenCalledWith('req-1', false)

    // Toast should have been shown
    expect(mockToastError).toHaveBeenCalledWith('Permission for Bash auto-denied', {
      description: 'Timed out waiting for response',
    })
  })

  it('does NOT show toast when user manually responds before timeout', () => {
    const onRespond = vi.fn()
    const { getByRole } = render(
      <PermissionCard permission={makePermission({ timeoutMs: 10000 })} onRespond={onRespond} />,
    )

    // Click Allow before timeout
    act(() => {
      getByRole('button', { name: /allow/i }).click()
    })

    // Advance past timeout
    act(() => {
      vi.advanceTimersByTime(11000)
    })

    expect(mockToastError).not.toHaveBeenCalled()
  })

  it('disables buttons and shows spinner when isPending (Fix 2)', () => {
    render(<PermissionCard permission={makePermission()} onRespond={vi.fn()} isPending />)

    const allowBtn = screen.getByRole('button', { name: /allow/i })
    const denyBtn = screen.getByRole('button', { name: /deny/i })

    expect(allowBtn).toBeDisabled()
    expect(denyBtn).toBeDisabled()
  })

  it('renders "Always Allow" button when suggestions present (Fix 5)', () => {
    const perm = makePermission({
      suggestions: [{ type: 'allow_tool', toolName: 'Bash' }],
    })
    render(<PermissionCard permission={perm} onRespond={vi.fn()} onAlwaysAllow={vi.fn()} />)

    expect(screen.getByRole('button', { name: /always allow/i })).toBeInTheDocument()
  })

  it('calls onAlwaysAllow with suggestion when "Always Allow" clicked', () => {
    const suggestion = { type: 'allow_tool', toolName: 'Bash' }
    const perm = makePermission({ suggestions: [suggestion] })
    const onAlwaysAllow = vi.fn()
    render(<PermissionCard permission={perm} onRespond={vi.fn()} onAlwaysAllow={onAlwaysAllow} />)

    act(() => {
      screen.getByRole('button', { name: /always allow/i }).click()
    })

    expect(onAlwaysAllow).toHaveBeenCalledWith('req-1', true, [suggestion])
  })

  it('does NOT render "Always Allow" when no suggestions', () => {
    render(<PermissionCard permission={makePermission()} onRespond={vi.fn()} />)

    expect(screen.queryByRole('button', { name: /always allow/i })).not.toBeInTheDocument()
  })
})
