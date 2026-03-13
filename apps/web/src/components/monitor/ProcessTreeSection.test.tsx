import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { ProcessTreeSnapshot } from '../../types/generated/ProcessTreeSnapshot'
import { ClassifiedProcessRow } from './ClassifiedProcessRow'
import { ProcessTreeSection } from './ProcessTreeSection'

function makeTree(overrides: Partial<ProcessTreeSnapshot> = {}): ProcessTreeSnapshot {
  return {
    timestamp: 1_700_000_000,
    ecosystem: [],
    children: [],
    totals: {
      ecosystemCpu: 0,
      ecosystemMemory: 0,
      ecosystemCount: 0,
      childCpu: 0,
      childMemory: 0,
      childCount: 0,
      unparentedCount: 0,
      unparentedMemory: 0,
    },
    ...overrides,
  }
}

function makeEcosystemProc(pid: number): ProcessTreeSnapshot['ecosystem'][0] {
  return {
    pid,
    ppid: 99,
    name: 'claude',
    command: '/usr/local/bin/claude',
    category: 'ClaudeEcosystem',
    ecosystemTag: 'cli',
    cpuPercent: 5.0,
    memoryBytes: 200_000_000,
    uptimeSecs: 3600,
    startTime: 1_700_000_000,
    isUnparented: false,
    staleness: 'Active',
    descendantCount: 0,
    descendantCpu: 0,
    descendantMemory: 0,
    descendants: [],
    isSelf: false,
  }
}

describe('ProcessTreeSection', () => {
  it('renders ecosystem table when data present', () => {
    const tree = makeTree({
      ecosystem: [makeEcosystemProc(1234)],
      totals: {
        ecosystemCpu: 5.0,
        ecosystemMemory: 200_000_000,
        ecosystemCount: 1,
        childCpu: 0,
        childMemory: 0,
        childCount: 0,
        unparentedCount: 0,
        unparentedMemory: 0,
      },
    })
    render(<ProcessTreeSection tree={tree} freshAt={Date.now()} />)
    expect(screen.getByText('Claude Ecosystem')).toBeInTheDocument()
    expect(screen.getByText('1234')).toBeInTheDocument()
  })

  it('does not render child table when children is empty', () => {
    const tree = makeTree({ ecosystem: [makeEcosystemProc(1234)] })
    render(<ProcessTreeSection tree={tree} freshAt={Date.now()} />)
    expect(screen.queryByText('Child Processes')).not.toBeInTheDocument()
  })

  it('shows Live indicator when freshAt is recent', () => {
    const tree = makeTree()
    render(<ProcessTreeSection tree={tree} freshAt={Date.now()} />)
    expect(screen.getByText('Live')).toBeInTheDocument()
  })

  it('shows Stale indicator when freshAt is old', () => {
    const tree = makeTree()
    render(<ProcessTreeSection tree={tree} freshAt={Date.now() - 30_000} />)
    expect(screen.getByText('Stale')).toBeInTheDocument()
  })

  it('shows Stale indicator when freshAt is null', () => {
    const tree = makeTree()
    render(<ProcessTreeSection tree={tree} freshAt={null} />)
    expect(screen.getByText('Stale')).toBeInTheDocument()
  })
})

describe('UnparentedBanner', () => {
  it('is hidden when unparentedCount is 0', () => {
    const tree = makeTree()
    render(<ProcessTreeSection tree={tree} freshAt={Date.now()} />)
    expect(screen.queryByText(/unparented/)).not.toBeInTheDocument()
  })

  it('shows count and memory when unparented processes exist', () => {
    const tree = makeTree({
      totals: {
        ecosystemCpu: 0,
        ecosystemMemory: 0,
        ecosystemCount: 0,
        childCpu: 0,
        childMemory: 0,
        childCount: 0,
        unparentedCount: 3,
        unparentedMemory: 15_000_000,
      },
    })
    render(<ProcessTreeSection tree={tree} freshAt={Date.now()} />)
    expect(screen.getByText(/3 unparented/)).toBeInTheDocument()
    expect(screen.getByText(/15 MB/)).toBeInTheDocument()
  })
})

describe('ClassifiedProcessRow kill button', () => {
  it('is disabled with tooltip when isSelf is true', () => {
    const tree = makeTree({
      ecosystem: [{ ...makeEcosystemProc(9999), isSelf: true }],
      totals: {
        ecosystemCpu: 0,
        ecosystemMemory: 0,
        ecosystemCount: 1,
        childCpu: 0,
        childMemory: 0,
        childCount: 0,
        unparentedCount: 0,
        unparentedMemory: 0,
      },
    })
    render(<ProcessTreeSection tree={tree} freshAt={Date.now()} />)
    const killButton = screen.getByTitle('Cannot kill this server process')
    expect(killButton).toBeDisabled()
  })
})

describe('ProcessTreeSection kill and cleanup endpoints', () => {
  it('calls kill callback when user opens dropdown, clicks Terminate, then confirms', async () => {
    const user = userEvent.setup()
    const proc = makeEcosystemProc(1234)
    const onKill = vi.fn()
    render(<ClassifiedProcessRow process={proc} onKill={onKill} />)

    // Open the dropdown via userEvent (Radix requires pointer events)
    const actionsButton = screen.getByTitle('Process actions')
    await user.click(actionsButton)

    // Radix portals the content into document.body — findByText searches the whole document
    const terminateButton = await screen.findByText('Terminate')
    await user.click(terminateButton)

    // Confirm dialog should appear
    const confirmButton = await screen.findByText('Yes')
    await user.click(confirmButton)

    expect(onKill).toHaveBeenCalledWith(1234, proc.startTime, false)
  })

  it('calls cleanup endpoint when cleanup button clicked', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve({}) })
    vi.stubGlobal('fetch', fetchMock)

    const staleProc = {
      ...makeEcosystemProc(5678),
      isUnparented: true,
      staleness: 'LikelyStale' as const,
    }
    const tree = makeTree({
      ecosystem: [staleProc],
      totals: {
        ecosystemCpu: 0,
        ecosystemMemory: 0,
        ecosystemCount: 1,
        childCpu: 0,
        childMemory: 0,
        childCount: 0,
        unparentedCount: 1,
        unparentedMemory: 100_000_000,
      },
    })
    render(<ProcessTreeSection tree={tree} freshAt={Date.now()} />)

    const cleanupButton = screen.getByText('Clean up stale')
    fireEvent.click(cleanupButton)

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        '/api/processes/cleanup',
        expect.objectContaining({ method: 'POST' }),
      )
    })
  })
})
