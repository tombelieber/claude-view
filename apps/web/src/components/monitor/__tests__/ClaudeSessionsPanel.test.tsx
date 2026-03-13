import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ClassifiedProcess } from '../../../types/generated/ClassifiedProcess'
import type { ProcessTreeSnapshot } from '../../../types/generated/ProcessTreeSnapshot'
import type { SessionResource } from '../../../types/generated/SessionResource'
import type { SystemInfo } from '../../../types/generated/SystemInfo'
import type { LiveSession } from '../../live/use-live-sessions'
import { ClaudeSessionsPanel } from '../ClaudeSessionsPanel'

const systemInfo: SystemInfo = {
  hostname: 'test',
  os: 'macOS',
  osVersion: '15.0',
  arch: 'aarch64',
  cpuCoreCount: 10,
  totalMemoryBytes: 32_000_000_000,
}

function makeResource(
  sessionId: string,
  pid: number,
  cpu = 50,
  mem = 500_000_000,
): SessionResource {
  return { sessionId, pid, cpuPercent: cpu, memoryBytes: mem }
}

function makeLiveSession(
  id: string,
  pid: number | null,
  overrides: Partial<LiveSession> = {},
): LiveSession {
  return {
    id,
    status: 'working',
    projectDisplayName: `project-${id}`,
    project: `project-${id}`,
    projectPath: `/code/${id}`,
    filePath: '',
    effectiveBranch: 'main',
    gitBranch: 'main',
    worktreeBranch: null,
    isWorktree: false,
    pid,
    title: '',
    lastUserMessage: '',
    currentActivity: '',
    turnCount: 10,
    startedAt: null,
    lastActivityAt: Date.now(),
    model: null,
    tokens: {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      totalTokens: 10000,
    },
    contextWindowTokens: 200000,
    cost: {
      totalUsd: 1.23,
      inputCostUsd: 0.5,
      outputCostUsd: 0.5,
      cacheReadCostUsd: 0.2,
      cacheCreationCostUsd: 0.03,
      cacheSavingsUsd: 0,
      hasUnpricedUsage: false,
      unpricedInputTokens: 0,
      unpricedOutputTokens: 0,
      unpricedCacheReadTokens: 0,
      unpricedCacheCreationTokens: 0,
      pricedTokenCoverage: 1,
      totalCostSource: 'api',
    },
    cacheStatus: 'warm',
    closedAt: null,
    editCount: 0,
    agentState: { type: 'tool_use' },
    ...overrides,
  } as LiveSession
}

function makeEcosystem(pid: number, overrides: Partial<ClassifiedProcess> = {}): ClassifiedProcess {
  return {
    pid,
    ppid: 1,
    name: 'claude',
    command: 'claude',
    category: 'ClaudeEcosystem',
    ecosystemTag: 'cli',
    cpuPercent: 10,
    memoryBytes: 250_000_000,
    uptimeSecs: 120,
    startTime: Date.now() / 1000 - 120,
    isUnparented: false,
    staleness: 'Active',
    descendantCount: 2,
    descendantCpu: 30,
    descendantMemory: 300_000_000,
    descendants: [
      {
        pid: pid + 1,
        ppid: pid,
        name: 'cargo',
        command: 'cargo build',
        category: 'ChildProcess',
        ecosystemTag: null,
        cpuPercent: 20,
        memoryBytes: 200_000_000,
        uptimeSecs: 60,
        startTime: Date.now() / 1000 - 60,
        isUnparented: false,
        staleness: 'Active',
        descendantCount: 0,
        descendantCpu: 0,
        descendantMemory: 0,
        descendants: [],
        isSelf: false,
      },
    ],
    isSelf: false,
    ...overrides,
  }
}

function makeProcessTree(
  ecosystem: ClassifiedProcess[],
  orphans: ClassifiedProcess[] = [],
): ProcessTreeSnapshot {
  return {
    timestamp: Date.now() / 1000,
    ecosystem,
    children: [],
    totals: {
      ecosystemCpu: ecosystem.reduce((s, p) => s + p.cpuPercent + p.descendantCpu, 0),
      ecosystemMemory: ecosystem.reduce((s, p) => s + p.memoryBytes + p.descendantMemory, 0),
      ecosystemCount: ecosystem.length,
      childCpu: 0,
      childMemory: 0,
      childCount: 0,
      unparentedCount: orphans.length,
      unparentedMemory: orphans.reduce((s, p) => s + p.memoryBytes, 0),
    },
  }
}

describe('ClaudeSessionsPanel', () => {
  it('renders session count in header', () => {
    const resources = [makeResource('s1', 100), makeResource('s2', 200)]
    const sessions = [makeLiveSession('s1', 100), makeLiveSession('s2', 200)]
    const tree = makeProcessTree([makeEcosystem(100), makeEcosystem(200)])

    render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    expect(screen.getByText('2')).toBeInTheDocument()
  })

  it('renders empty state when no sessions', () => {
    render(
      <ClaudeSessionsPanel
        sessionResources={[]}
        liveSessions={[]}
        processTree={null}
        systemInfo={null}
      />,
    )

    expect(screen.getByText('No active Claude sessions')).toBeInTheDocument()
  })

  it('two-step merge: sessionId then PID', () => {
    const resources = [makeResource('s1', 100)]
    const sessions = [makeLiveSession('s1', 100)]
    const eco = makeEcosystem(100)
    const tree = makeProcessTree([eco])

    render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    // Session name rendered
    expect(screen.getByText('project-s1')).toBeInTheDocument()
    // Ecosystem badge
    expect(screen.getByText('CLI')).toBeInTheDocument()
  })

  it('handles null PID gracefully', () => {
    const resources = [makeResource('s1', 100)]
    const sessions = [makeLiveSession('s1', null)]
    const tree = makeProcessTree([])

    render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    // Still renders with the session name (resource data provides cpu/mem)
    expect(screen.getByText('project-s1')).toBeInTheDocument()
  })

  it('rollup header CPU bar exists', () => {
    const resources = [makeResource('s1', 100)]
    const sessions = [makeLiveSession('s1', 100)]
    const tree = makeProcessTree([makeEcosystem(100)])

    const { container } = render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    // Header has a progressbar for rollup
    const progressBars = container.querySelectorAll('[role="progressbar"]')
    expect(progressBars.length).toBeGreaterThanOrEqual(1)
  })

  it('sorts sessions by rollup CPU desc', () => {
    // s1: cpu 20 + descendantCpu 0 = 20
    // s2: cpu 80 + descendantCpu 30 = 110
    const resources = [makeResource('s1', 100, 20), makeResource('s2', 200, 80)]
    const sessions = [makeLiveSession('s1', 100), makeLiveSession('s2', 200)]
    const eco2 = makeEcosystem(200, { descendantCpu: 30 })
    const tree = makeProcessTree([makeEcosystem(100, { descendantCpu: 0, descendants: [] }), eco2])

    render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    const names = screen.getAllByText(/^project-/)
    // s2 (higher CPU) should come first
    expect(names[0].textContent).toBe('project-s2')
    expect(names[1].textContent).toBe('project-s1')
  })

  it('expand all toggles all rows', () => {
    const resources = [makeResource('s1', 100), makeResource('s2', 200)]
    const sessions = [makeLiveSession('s1', 100), makeLiveSession('s2', 200)]
    const eco1 = makeEcosystem(100)
    const eco2 = makeEcosystem(200)
    const tree = makeProcessTree([eco1, eco2])

    render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    // Click "Expand All"
    const expandBtn = screen.getByRole('button', { name: /expand all/i })
    fireEvent.click(expandBtn)

    // Child processes from both ecosystems should be visible
    const cargoElements = screen.getAllByText('cargo')
    expect(cargoElements.length).toBe(2)
  })

  it('collapse all toggles back', () => {
    const resources = [makeResource('s1', 100), makeResource('s2', 200)]
    const sessions = [makeLiveSession('s1', 100), makeLiveSession('s2', 200)]
    const eco1 = makeEcosystem(100)
    const eco2 = makeEcosystem(200)
    const tree = makeProcessTree([eco1, eco2])

    render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    // Expand all first
    const expandBtn = screen.getByRole('button', { name: /expand all/i })
    fireEvent.click(expandBtn)

    // Now it should say "Collapse All"
    const collapseBtn = screen.getByRole('button', { name: /collapse all/i })
    fireEvent.click(collapseBtn)

    // Children should be hidden
    expect(screen.queryByText('cargo')).not.toBeInTheDocument()
  })

  it('orphan row shown when orphans exist', () => {
    const orphan = makeEcosystem(999, {
      isUnparented: true,
      staleness: 'LikelyStale',
      descendantCount: 0,
      descendantCpu: 0,
      descendantMemory: 0,
      descendants: [],
    })
    const tree = makeProcessTree([], [orphan])
    // Put orphan in ecosystem array with isUnparented=true
    tree.ecosystem = [orphan]

    render(
      <ClaudeSessionsPanel
        sessionResources={[]}
        liveSessions={[]}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    expect(screen.getByText(/Orphaned Processes/)).toBeInTheDocument()
  })

  it('orphan cleanup button calls fetch', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue({ ok: true, json: () => Promise.resolve({ killed: [999], failed: [] }) })
    vi.stubGlobal('fetch', fetchMock)

    const orphan = makeEcosystem(999, {
      isUnparented: true,
      staleness: 'LikelyStale',
      isSelf: false,
      descendantCount: 0,
      descendantCpu: 0,
      descendantMemory: 0,
      descendants: [],
    })
    const tree = makeProcessTree([], [orphan])
    tree.ecosystem = [orphan]

    render(
      <ClaudeSessionsPanel
        sessionResources={[]}
        liveSessions={[]}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    const cleanupBtn = screen.getByRole('button', { name: /clean up/i })
    fireEvent.click(cleanupBtn)

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        '/api/processes/cleanup',
        expect.objectContaining({ method: 'POST' }),
      )
    })

    vi.unstubAllGlobals()
  })

  it('auto-expands new rows when all rows are already expanded', () => {
    const resources1 = [makeResource('s1', 100)]
    const sessions1 = [makeLiveSession('s1', 100)]
    const eco1 = makeEcosystem(100)
    const tree1 = makeProcessTree([eco1])

    const { rerender } = render(
      <ClaudeSessionsPanel
        sessionResources={resources1}
        liveSessions={sessions1}
        processTree={tree1}
        systemInfo={systemInfo}
      />,
    )

    // Expand all (just 1 session)
    fireEvent.click(screen.getByRole('button', { name: /expand all/i }))
    expect(screen.getByText('cargo')).toBeInTheDocument()

    // Now add a second session — it should auto-expand since all rows were expanded
    const resources2 = [makeResource('s1', 100), makeResource('s2', 200)]
    const sessions2 = [makeLiveSession('s1', 100), makeLiveSession('s2', 200)]
    const eco2 = makeEcosystem(200, {
      descendants: [
        {
          pid: 201,
          ppid: 200,
          name: 'vitest',
          command: 'vitest run',
          category: 'ChildProcess',
          ecosystemTag: null,
          cpuPercent: 15,
          memoryBytes: 100_000_000,
          uptimeSecs: 30,
          startTime: Date.now() / 1000 - 30,
          isUnparented: false,
          staleness: 'Active',
          descendantCount: 0,
          descendantCpu: 0,
          descendantMemory: 0,
          descendants: [],
          isSelf: false,
        },
      ],
    })
    const tree2 = makeProcessTree([eco1, eco2])

    rerender(
      <ClaudeSessionsPanel
        sessionResources={resources2}
        liveSessions={sessions2}
        processTree={tree2}
        systemInfo={systemInfo}
      />,
    )

    // The new session's child process should be visible (auto-expanded)
    expect(screen.getByText('vitest')).toBeInTheDocument()
    // Old session's child should still be visible too
    expect(screen.getByText('cargo')).toBeInTheDocument()
  })

  it('kill process callback invoked via fetch', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ killed: true, pid: 101, error: null }),
      })
    vi.stubGlobal('fetch', fetchMock)

    const resources = [makeResource('s1', 100)]
    const sessions = [makeLiveSession('s1', 100)]
    const eco = makeEcosystem(100)
    const tree = makeProcessTree([eco])

    render(
      <ClaudeSessionsPanel
        sessionResources={resources}
        liveSessions={sessions}
        processTree={tree}
        systemInfo={systemInfo}
      />,
    )

    // Expand the session to see child processes
    const toggleBtn = screen.getByRole('button', { name: /toggle/i })
    fireEvent.click(toggleBtn)

    // Find the kill X button on the child process row
    // The ChildProcessRow shows an X button on hover; click it to trigger confirm
    const killButtons = screen.getAllByTitle('Terminate process')
    fireEvent.click(killButtons[0])

    // Confirm kill
    const yesBtn = screen.getByText('Yes')
    fireEvent.click(yesBtn)

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        '/api/processes/101/kill',
        expect.objectContaining({ method: 'POST' }),
      )
    })

    vi.unstubAllGlobals()
  })
})
