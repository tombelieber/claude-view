// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ClaudeHomeEntry } from '../../types/generated/ClaudeHomeEntry'
import type { WorkflowAgentDetail } from '../../types/generated/WorkflowAgentDetail'
import type { WorkflowAgentSummary } from '../../types/generated/WorkflowAgentSummary'
import type { WorkflowPhaseSummary } from '../../types/generated/WorkflowPhaseSummary'
import type { WorkflowRunDetail } from '../../types/generated/WorkflowRunDetail'
import type { WorkflowRunSummary } from '../../types/generated/WorkflowRunSummary'
import { ClaudeHomePage } from '../ClaudeHomePage'
import { WorkflowRunDetailPage } from '../WorkflowRunDetailPage'
import { WorkflowsPage } from '../WorkflowsPage'

// react-virtuoso cannot measure height under happy-dom and would render zero
// items, so stub it to render every item synchronously (these tests assert on
// list content, not virtualization behaviour).
vi.mock('react-virtuoso', () => ({
  Virtuoso: ({
    data = [],
    itemContent,
  }: {
    data?: unknown[]
    itemContent: (index: number, item: unknown) => ReactElement
  }) => (
    <div data-testid="virtuoso">
      {data.map((item, index) => (
        <div key={index}>{itemContent(index, item)}</div>
      ))}
    </div>
  ),
}))

const mockState = vi.hoisted(() => ({
  runsResponse: { runs: [] } as { runs: WorkflowRunSummary[] },
  runsLoading: false,
  runsError: false,
  runDetail: undefined as WorkflowRunDetail | undefined,
  runDetailLoading: false,
  runDetailError: false,
  agentDetail: undefined as WorkflowAgentDetail | undefined,
  homeEntries: [] as ClaudeHomeEntry[],
  homeLoading: false,
}))

vi.mock('../../hooks/use-workflows', () => ({
  useWorkflowRuns: () => ({
    data: mockState.runsResponse,
    isLoading: mockState.runsLoading,
    isError: mockState.runsError,
  }),
  useWorkflowRun: () => ({
    data: mockState.runDetail,
    isLoading: mockState.runDetailLoading,
    isError: mockState.runDetailError,
  }),
  useWorkflowAgent: () => ({
    data: mockState.agentDetail,
  }),
  useClaudeHomeEntries: () => ({
    data: mockState.homeEntries,
    isLoading: mockState.homeLoading,
    isError: false,
  }),
}))

function renderWithProviders(ui: ReactElement, initialEntries = ['/workflows']) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={initialEntries}>{ui}</MemoryRouter>
    </QueryClientProvider>,
  )
}

const baseRuns: WorkflowRunSummary[] = [
  {
    sessionId: 'sess-hermes',
    runId: 'wf_hermes',
    projectDir: 'claude-view',
    workflowName: 'Hermes Design',
    status: 'completed',
    summary: 'Mapped agent-first workflow architecture',
    defaultModel: 'claude-sonnet-4-5',
    startTime: Date.now() - 3_600_000,
    durationMs: 92_000,
    totalTokens: 123_456,
    totalToolCalls: 34,
    agentCount: 4,
    phaseCount: 1,
    updatedAt: Date.now() - 1_800_000,
    scriptPreview: 'workflow.define(...)',
    resultPreview: 'Architecture complete',
    hasSummaryJson: true,
    hasJournal: true,
  },
  {
    sessionId: 'sess-live',
    runId: 'wf_live',
    projectDir: 'design-system',
    workflowName: 'Live Refactor',
    status: 'running',
    summary: 'Still collecting agent output',
    defaultModel: 'claude-opus-4-5',
    startTime: Date.now() - 600_000,
    durationMs: null,
    totalTokens: 42_000,
    totalToolCalls: 8,
    agentCount: 2,
    phaseCount: 1,
    updatedAt: Date.now() - 60_000,
    scriptPreview: null,
    resultPreview: null,
    hasSummaryJson: false,
    hasJournal: true,
  },
]

const phase: WorkflowPhaseSummary = {
  index: 0,
  title: 'Map',
  detail: 'Map the repo and Claude artifacts',
  agentCount: 1,
  completedAgentCount: 1,
  tokenCount: 12_345,
  toolCallCount: 7,
  durationMs: 12_000,
}

const agent: WorkflowAgentSummary = {
  agentId: 'agent-scout',
  label: 'Scout agent',
  phaseIndex: 0,
  phaseTitle: 'Map',
  model: 'claude-sonnet-4-5',
  state: 'completed',
  startedAt: null,
  queuedAt: null,
  lastProgressAt: null,
  tokens: 12_345,
  toolCalls: 7,
  durationMs: 12_000,
  promptPreview: 'Inspect Claude Code artifacts',
  resultPreview: 'Found workflow JSONL files',
  eventsAvailable: true,
}

beforeEach(() => {
  mockState.runsResponse = { runs: [...baseRuns] }
  mockState.runsLoading = false
  mockState.runsError = false
  mockState.runDetail = undefined
  mockState.runDetailLoading = false
  mockState.runDetailError = false
  mockState.agentDetail = undefined
  mockState.homeEntries = []
  mockState.homeLoading = false
  vi.clearAllMocks()
})

describe('WorkflowsPage', () => {
  it('shows workflow run loading state', () => {
    mockState.runsLoading = true

    renderWithProviders(<WorkflowsPage />)

    expect(screen.getByText('Loading workflow runs...')).toBeInTheDocument()
  })

  it('loads real workflow runs by default and filters them', async () => {
    const user = userEvent.setup()
    renderWithProviders(<WorkflowsPage />)

    expect(screen.getByRole('heading', { name: 'Workflows' })).toBeInTheDocument()
    expect(screen.getByText('Hermes Design')).toBeInTheDocument()
    expect(screen.getByText('Live Refactor')).toBeInTheDocument()

    await user.type(screen.getByPlaceholderText('Search runs, sessions, projects'), 'live')

    expect(screen.queryByText('Hermes Design')).not.toBeInTheDocument()
    expect(screen.getByText('Live Refactor')).toBeInTheDocument()

    await user.selectOptions(screen.getByDisplayValue('All statuses'), 'completed')

    expect(screen.getByText('No workflow runs found')).toBeInTheDocument()
  })

  it('shows an empty state when no Claude workflow artifacts exist', () => {
    mockState.runsResponse = { runs: [] }

    renderWithProviders(<WorkflowsPage />)

    expect(screen.getByText('No workflow runs found')).toBeInTheDocument()
    expect(screen.getByText(/~\/.claude\/projects/)).toBeInTheDocument()
  })

  it('surfaces a read error instead of masquerading as empty', () => {
    mockState.runsResponse = { runs: [] }
    mockState.runsError = true

    renderWithProviders(<WorkflowsPage />)

    expect(screen.getByText(/Could not read workflow runs/)).toBeInTheDocument()
  })

  it('no longer surfaces the legacy YAML definitions tab', () => {
    renderWithProviders(<WorkflowsPage />)

    expect(screen.queryByRole('button', { name: 'Legacy definitions' })).not.toBeInTheDocument()
    expect(screen.queryByText('Official definitions')).not.toBeInTheDocument()
  })
})

describe('WorkflowRunDetailPage', () => {
  it('renders phases, agent detail, script, result, and parent session link', () => {
    mockState.runDetail = {
      summary: baseRuns[0],
      phases: [phase],
      agents: [agent],
      script: 'export default async function workflow() {\n  console.log("run")\n}',
      result: 'Workflow completed with mapped artifacts.',
      journal: [
        { kind: 'agent_complete', agentId: 'agent-scout', preview: 'done', timestamp: null },
      ],
      artifactRelativePath: 'projects/project/sess-hermes/workflows/wf_hermes.json',
    }
    mockState.agentDetail = {
      summary: agent,
      promptPreview: 'Prompt text from workflow JSONL',
      resultPreview: 'Result text from workflow JSONL',
      events: [
        { kind: 'tool_call', role: 'assistant', preview: 'Tool call preview', timestamp: null },
      ],
      metaPreview: null,
    }

    renderWithProviders(
      <Routes>
        <Route path="/workflows/runs/:sessionId/:runId" element={<WorkflowRunDetailPage />} />
      </Routes>,
      ['/workflows/runs/sess-hermes/wf_hermes'],
    )

    expect(screen.getByRole('heading', { name: 'Hermes Design' })).toBeInTheDocument()
    expect(screen.getByText('Parent session')).toBeInTheDocument()
    expect(screen.getByText('1. Map')).toBeInTheDocument()
    expect(screen.getAllByText('Scout agent').length).toBeGreaterThan(0)
    expect(screen.getByText(/console\.log/)).toBeInTheDocument()
    expect(screen.getByText('Workflow completed with mapped artifacts.')).toBeInTheDocument()
    expect(screen.getByText('Prompt text from workflow JSONL')).toBeInTheDocument()
    expect(screen.getByText('Tool call preview')).toBeInTheDocument()
  })

  it('shows an error state when the run cannot be loaded', () => {
    mockState.runDetail = undefined
    mockState.runDetailError = true

    renderWithProviders(
      <Routes>
        <Route path="/workflows/runs/:sessionId/:runId" element={<WorkflowRunDetailPage />} />
      </Routes>,
      ['/workflows/runs/sess-x/wf_x'],
    )

    expect(screen.getByText(/Could not load this workflow run/)).toBeInTheDocument()
  })
})

describe('ClaudeHomePage', () => {
  it('renders metadata-only Claude home areas without leaking sensitive previews', () => {
    mockState.homeEntries = [
      {
        kind: 'session-env',
        name: 'session-env',
        relativePath: 'session-env',
        path: '/Users/test/.claude/session-env',
        isDirectory: true,
        itemCount: 2,
        sizeBytes: 128,
        modifiedAt: 1,
        preview: null,
        previewTruncated: false,
        metadataOnly: true,
      },
      {
        kind: 'hooks',
        name: 'stop_gate.py',
        relativePath: 'hooks/stop_gate.py',
        path: '/Users/test/.claude/hooks/stop_gate.py',
        isDirectory: false,
        itemCount: 0,
        sizeBytes: 64,
        modifiedAt: 2,
        preview: 'hook body preview',
        previewTruncated: false,
        metadataOnly: false,
      },
    ]

    renderWithProviders(<ClaudeHomePage />, ['/claude-home'])

    expect(screen.getByRole('heading', { name: 'Claude Home' })).toBeInTheDocument()
    expect(screen.getAllByText('session-env').length).toBeGreaterThan(0)
    expect(screen.getAllByText('stop_gate.py').length).toBeGreaterThan(0)
    expect(screen.getByText('hook body preview')).toBeInTheDocument()
    expect(screen.getAllByText('metadata-only').length).toBeGreaterThan(0)
    expect(screen.queryByText(/SECRET_TOKEN/)).not.toBeInTheDocument()
  })
})
