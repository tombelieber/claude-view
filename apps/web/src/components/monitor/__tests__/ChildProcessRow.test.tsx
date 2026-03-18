import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ClassifiedProcess } from '../../../types/generated/ClassifiedProcess'
import type { SystemInfo } from '../../../types/generated/SystemInfo'
import { ChildProcessRow } from '../ChildProcessRow'

const systemInfo: SystemInfo = {
  hostname: 'test-machine',
  os: 'macOS',
  osVersion: '15.0',
  arch: 'aarch64',
  cpuCoreCount: 10,
  totalMemoryBytes: 32_000_000_000,
}

function makeProcess(overrides: Partial<ClassifiedProcess> = {}): ClassifiedProcess {
  return {
    pid: 1234,
    ppid: 100,
    name: 'cargo build',
    command: 'cargo build --release',
    category: 'ChildProcess',
    ecosystemTag: null,
    cpuPercent: 52.0,
    memoryBytes: 340_000_000,
    uptimeSecs: 120,
    startTime: Date.now() / 1000 - 120,
    isUnparented: false,
    staleness: 'Active',
    descendantCount: 0,
    descendantCpu: 0,
    descendantMemory: 0,
    descendants: [],
    isSelf: false,
    ...overrides,
  }
}

describe('ChildProcessRow', () => {
  it('renders process name, CPU%, RAM, and age', () => {
    render(
      <ChildProcessRow
        process={makeProcess()}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    expect(screen.getByText('cargo build')).toBeInTheDocument()
    expect(screen.getByText('5.2%')).toBeInTheDocument() // 52/10 cores = 5.2% of system
    expect(screen.getByText('324 MB')).toBeInTheDocument() // binary: 340_000_000 / 1024^2
    expect(screen.getByText('2m')).toBeInTheDocument()
  })

  it('shows normalized CPU bar (per-core -> system %)', () => {
    const { container } = render(
      <ChildProcessRow
        process={makeProcess({ cpuPercent: 200 })} // 2 full cores
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    // 200 / 10 = 20% of system
    const bar = container.querySelector('[role="progressbar"]')
    expect(bar).toHaveAttribute('aria-valuenow', '20')
  })

  it('shows green age for processes under 60s', () => {
    const { container } = render(
      <ChildProcessRow
        process={makeProcess({ uptimeSecs: 30 })}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    const ageEl = container.querySelector('[data-testid="process-age"]')
    expect(ageEl?.className).toContain('text-green')
  })

  it('shows amber age for idle processes over 5m', () => {
    const { container } = render(
      <ChildProcessRow
        process={makeProcess({ uptimeSecs: 600, staleness: 'Idle' })}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    const ageEl = container.querySelector('[data-testid="process-age"]')
    expect(ageEl?.className).toContain('text-amber')
  })

  it('calls onKill with correct args when kill confirmed', () => {
    const onKill = vi.fn()
    const proc = makeProcess({ pid: 5678, startTime: 1000 })
    render(
      <ChildProcessRow
        process={proc}
        systemInfo={systemInfo}
        onKill={onKill}
        pendingPids={new Set()}
      />,
    )
    // Click the kill button to show confirmation
    fireEvent.click(screen.getByTitle('Terminate process'))
    // Confirm
    fireEvent.click(screen.getByText('Yes'))
    expect(onKill).toHaveBeenCalledWith(5678, 1000, false)
  })

  it('disables kill button when isSelf', () => {
    render(
      <ChildProcessRow
        process={makeProcess({ isSelf: true })}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    const killBtn = screen.getByTitle('Cannot kill this server process')
    expect(killBtn).toBeDisabled()
  })

  it('shows killing indicator when isPending', () => {
    const proc = makeProcess({ pid: 1234 })
    render(
      <ChildProcessRow
        process={proc}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set([1234])}
      />,
    )
    // When isPending, the component shows "Killing <pid>..." instead of the kill button
    expect(screen.getByText(/Killing 1234/)).toBeInTheDocument()
  })

  it('shows chevron and child count when descendants exist', () => {
    const proc = makeProcess({
      descendants: [
        makeProcess({ pid: 2001, name: 'node worker' }),
        makeProcess({ pid: 2002, name: 'tsc --watch' }),
      ],
      descendantCount: 2,
    })
    render(
      <ChildProcessRow
        process={proc}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    expect(screen.getByLabelText('Toggle child processes')).toBeInTheDocument()
    expect(screen.getByText('+2')).toBeInTheDocument()
  })

  it('expands to show nested children on click', () => {
    const proc = makeProcess({
      descendants: [makeProcess({ pid: 2001, name: 'node worker' })],
      descendantCount: 1,
    })
    render(
      <ChildProcessRow
        process={proc}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    // Children not visible initially
    expect(screen.queryByText('node worker')).not.toBeInTheDocument()
    // Expand
    fireEvent.click(screen.getByLabelText('Toggle child processes'))
    expect(screen.getByText('node worker')).toBeInTheDocument()
  })

  it('shows spacer instead of chevron when no descendants', () => {
    render(
      <ChildProcessRow
        process={makeProcess()}
        systemInfo={systemInfo}
        onKill={vi.fn()}
        pendingPids={new Set()}
      />,
    )
    expect(screen.queryByLabelText('Toggle child processes')).not.toBeInTheDocument()
  })
})
