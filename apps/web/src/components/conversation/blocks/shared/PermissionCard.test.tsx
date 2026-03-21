// Tests for PermissionCard — guards the critical permission request/response flow.
// This is a turn-blocking interaction: bugs here break the chat pipeline.

import type { PermissionRequest } from '@claude-view/shared/types/sidecar-protocol'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PermissionCard } from './PermissionCard'

function makePermission(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    type: 'permission_request',
    requestId: 'req-1',
    toolName: 'Bash',
    toolInput: { command: 'ls -la' },
    toolUseID: 'tool-use-1',
    decisionReason: 'Needs shell access',
    timeoutMs: 30000,
    suggestions: [],
    ...overrides,
  }
}

describe('PermissionCard', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders tool name and command content', () => {
    render(<PermissionCard permission={makePermission()} />)

    expect(screen.getByText('Bash')).toBeInTheDocument()
    expect(screen.getByText('ls -la')).toBeInTheDocument()
    expect(screen.getByText('Needs shell access')).toBeInTheDocument()
  })

  it('shows Allow and Deny buttons when onRespond is provided', () => {
    const onRespond = vi.fn()
    render(<PermissionCard permission={makePermission()} onRespond={onRespond} />)

    expect(screen.getByText('Allow')).toBeInTheDocument()
    expect(screen.getByText('Deny')).toBeInTheDocument()
  })

  it('hides action buttons when onRespond is not provided (resolved state)', () => {
    render(<PermissionCard permission={makePermission()} />)

    expect(screen.queryByText('Allow')).not.toBeInTheDocument()
    expect(screen.queryByText('Deny')).not.toBeInTheDocument()
  })

  it('calls onRespond(requestId, true) when Allow is clicked', () => {
    const onRespond = vi.fn()
    render(<PermissionCard permission={makePermission()} onRespond={onRespond} />)

    fireEvent.click(screen.getByText('Allow'))

    expect(onRespond).toHaveBeenCalledWith('req-1', true)
    expect(onRespond).toHaveBeenCalledTimes(1)
  })

  it('calls onRespond(requestId, false) when Deny is clicked', () => {
    const onRespond = vi.fn()
    render(<PermissionCard permission={makePermission()} onRespond={onRespond} />)

    fireEvent.click(screen.getByText('Deny'))

    expect(onRespond).toHaveBeenCalledWith('req-1', false)
    expect(onRespond).toHaveBeenCalledTimes(1)
  })

  it('shows "Always Allow" only when suggestions exist and onAlwaysAllow provided', () => {
    const onRespond = vi.fn()
    const onAlwaysAllow = vi.fn()
    const perm = makePermission({
      suggestions: [{ type: 'allow', toolName: 'Bash' }] as unknown[],
    })

    render(<PermissionCard permission={perm} onRespond={onRespond} onAlwaysAllow={onAlwaysAllow} />)

    expect(screen.getByText('Always Allow')).toBeInTheDocument()
  })

  it('hides "Always Allow" when no suggestions', () => {
    const onRespond = vi.fn()
    render(
      <PermissionCard permission={makePermission({ suggestions: [] })} onRespond={onRespond} />,
    )

    expect(screen.queryByText('Always Allow')).not.toBeInTheDocument()
  })

  it('shows countdown timer when not resolved', () => {
    const onRespond = vi.fn()
    render(
      <PermissionCard permission={makePermission({ timeoutMs: 30000 })} onRespond={onRespond} />,
    )

    expect(screen.getByText('30s')).toBeInTheDocument()
  })

  it('countdown ticks down each second', () => {
    const onRespond = vi.fn()
    render(
      <PermissionCard permission={makePermission({ timeoutMs: 5000 })} onRespond={onRespond} />,
    )

    expect(screen.getByText('5s')).toBeInTheDocument()
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(screen.getByText('4s')).toBeInTheDocument()
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(screen.getByText('3s')).toBeInTheDocument()
  })

  it('auto-denies on timeout (countdown reaches 0)', () => {
    const onRespond = vi.fn()
    render(
      <PermissionCard permission={makePermission({ timeoutMs: 2000 })} onRespond={onRespond} />,
    )

    act(() => {
      vi.advanceTimersByTime(2000)
    })

    expect(onRespond).toHaveBeenCalledWith('req-1', false)
  })

  it('hides countdown when resolved', () => {
    render(
      <PermissionCard
        permission={makePermission({ timeoutMs: 30000 })}
        resolved={{ allowed: true }}
      />,
    )

    expect(screen.queryByText('30s')).not.toBeInTheDocument()
  })

  it('disables buttons when isPending', () => {
    const onRespond = vi.fn()
    render(<PermissionCard permission={makePermission()} onRespond={onRespond} isPending />)

    expect(screen.getByText('Allow')).toBeDisabled()
    expect(screen.getByText('Deny')).toBeDisabled()
  })

  it('displays file path for Edit tool', () => {
    render(
      <PermissionCard
        permission={makePermission({
          toolName: 'Edit',
          toolInput: {
            file_path: '/src/main.ts',
            old_string: 'foo',
            new_string: 'bar',
          },
        })}
      />,
    )

    expect(screen.getByText('Edit')).toBeInTheDocument()
    expect(screen.getByText('File: /src/main.ts')).toBeInTheDocument()
  })

  it('displays file path for Read tool', () => {
    render(
      <PermissionCard
        permission={makePermission({
          toolName: 'Read',
          toolInput: { file_path: '/etc/hosts' },
        })}
      />,
    )

    expect(screen.getByText('/etc/hosts')).toBeInTheDocument()
  })
})
