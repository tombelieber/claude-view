import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ClassifiedProcess } from '../../../types/generated/ClassifiedProcess'
import type { SessionResource } from '../../../types/generated/SessionResource'
import type { SystemInfo } from '../../../types/generated/SystemInfo'
import type { LiveSession } from '../../live/use-live-sessions'
import { SessionAccordionRow } from '../SessionAccordionRow'

const systemInfo: SystemInfo = {
  hostname: 'test',
  os: 'macOS',
  osVersion: '15.0',
  arch: 'aarch64',
  cpuCoreCount: 10,
  totalMemoryBytes: 32_000_000_000,
}

const resource: SessionResource = {
  sessionId: 'sess-1',
  pid: 100,
  cpuPercent: 50.0,
  memoryBytes: 680_000_000,
}

function makeLiveSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'sess-1',
    status: 'working',
    projectDisplayName: 'my-api',
    project: 'my-api',
    projectPath: '/code/my-api',
    filePath: '',
    effectiveBranch: 'feat/auth',
    gitBranch: 'feat/auth',
    worktreeBranch: null,
    isWorktree: false,
    pid: 100,
    title: '',
    lastUserMessage: '',
    currentActivity: '',
    turnCount: 23,
    startedAt: null,
    lastActivityAt: Date.now(),
    model: null,
    tokens: {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      cacheCreation5mTokens: 0,
      cacheCreation1hrTokens: 0,
      totalTokens: 50000,
    },
    contextWindowTokens: 200000,
    cost: {
      totalUsd: 4.56,
      inputCostUsd: 2.0,
      outputCostUsd: 2.0,
      cacheReadCostUsd: 0.5,
      cacheCreationCostUsd: 0.06,
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

function makeEcosystem(overrides: Partial<ClassifiedProcess> = {}): ClassifiedProcess {
  return {
    pid: 100,
    ppid: 1,
    name: 'claude',
    command: 'claude',
    category: 'ClaudeEcosystem',
    ecosystemTag: 'cli',
    cpuPercent: 10.0,
    memoryBytes: 250_000_000,
    uptimeSecs: 120,
    startTime: Date.now() / 1000 - 120,
    isUnparented: false,
    staleness: 'Active',
    descendantCount: 3,
    descendantCpu: 40.0,
    descendantMemory: 430_000_000,
    descendants: [
      {
        pid: 101,
        ppid: 100,
        name: 'cargo build',
        command: 'cargo build',
        category: 'ChildProcess',
        ecosystemTag: null,
        cpuPercent: 25.0,
        memoryBytes: 340_000_000,
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

describe('SessionAccordionRow', () => {
  const defaultProps = {
    session: makeLiveSession(),
    resource,
    ecosystemProcess: makeEcosystem(),
    systemInfo,
    expanded: false,
    onToggle: vi.fn(),
    onKill: vi.fn(),
    pendingPids: new Set<number>(),
  }

  it('renders session name and branch', () => {
    render(<SessionAccordionRow {...defaultProps} />)
    expect(screen.getByText('my-api')).toBeInTheDocument()
    expect(screen.getByText('feat/auth')).toBeInTheDocument()
  })

  it('renders source badge for IDE session', () => {
    render(
      <SessionAccordionRow
        {...defaultProps}
        session={makeLiveSession({ source: { category: 'ide', label: 'VS Code' } })}
      />,
    )
    expect(screen.getByText('VS Code')).toBeInTheDocument()
  })

  it('renders source badge for agent SDK session', () => {
    render(
      <SessionAccordionRow
        {...defaultProps}
        session={makeLiveSession({ source: { category: 'agent_sdk', label: null } })}
      />,
    )
    expect(screen.getByText('This App')).toBeInTheDocument()
  })

  it('shows cost and turn count', () => {
    render(<SessionAccordionRow {...defaultProps} />)
    expect(screen.getByText('$4.56')).toBeInTheDocument()
    expect(screen.getByText(/23/)).toBeInTheDocument()
  })

  it('shows child count hint when collapsed', () => {
    render(<SessionAccordionRow {...defaultProps} expanded={false} />)
    expect(screen.getByText(/3 child/)).toBeInTheDocument()
  })

  it('hides child count hint when 0 children', () => {
    render(
      <SessionAccordionRow
        {...defaultProps}
        ecosystemProcess={makeEcosystem({ descendantCount: 0, descendants: [] })}
      />,
    )
    expect(screen.queryByText(/child/)).not.toBeInTheDocument()
  })

  it('toggles on click', () => {
    const onToggle = vi.fn()
    render(<SessionAccordionRow {...defaultProps} onToggle={onToggle} />)
    fireEvent.click(screen.getByRole('button', { name: /toggle/i }))
    expect(onToggle).toHaveBeenCalledTimes(1)
  })

  it('shows child processes when expanded', () => {
    render(<SessionAccordionRow {...defaultProps} expanded={true} />)
    expect(screen.getByText('cargo build')).toBeInTheDocument()
  })

  it('shows shimmer when ecosystemProcess is null and pid is known', () => {
    render(<SessionAccordionRow {...defaultProps} ecosystemProcess={null} expanded={true} />)
    expect(screen.getByText(/loading/i)).toBeInTheDocument()
  })

  it('shows PID unknown when ecosystemProcess is null and session.pid is null', () => {
    render(
      <SessionAccordionRow
        {...defaultProps}
        session={makeLiveSession({ pid: null })}
        ecosystemProcess={null}
        expanded={true}
      />,
    )
    expect(screen.getByText(/PID unknown/)).toBeInTheDocument()
    expect(screen.queryByText(/loading/i)).not.toBeInTheDocument()
  })

  it('computes rollup CPU = resource + descendantCpu', () => {
    // resource.cpuPercent = 50, ecosystem.descendantCpu = 40
    // rollup = 90, normalized = 90 / 10 = 9%
    const { container } = render(<SessionAccordionRow {...defaultProps} />)
    const cpuBars = container.querySelectorAll('[role="progressbar"]')
    // First progressbar should be CPU with rollup value
    const cpuBar = cpuBars[0]
    // 90 / (10 * 100) * 100 = 9% of system
    expect(cpuBar).toHaveAttribute('aria-valuenow', '9')
  })

  it('uses resource-only CPU when no ecosystem process', () => {
    const { container } = render(<SessionAccordionRow {...defaultProps} ecosystemProcess={null} />)
    const cpuBars = container.querySelectorAll('[role="progressbar"]')
    const cpuBar = cpuBars[0]
    // 50 / (10 * 100) * 100 = 5%
    expect(cpuBar).toHaveAttribute('aria-valuenow', '5')
  })

  it('renders status dot with working pulse', () => {
    const { container } = render(<SessionAccordionRow {...defaultProps} />)
    const dot = container.querySelector('[data-testid="status-dot"]')
    expect(dot?.className).toContain('bg-green-500')
    expect(dot?.className).toContain('animate-pulse')
  })
})
