import type { CostBreakdown as CostBreakdownType } from '@claude-view/shared/types/generated'
import type { TokenUsage } from '@claude-view/shared/types/generated'
import * as Tooltip from '@radix-ui/react-tooltip'
/**
 * Statusline UI Revamp — Design Canvas
 *
 * Shows ALL components that consume statusline data, with mock data filled
 * for every new field (rate limits, vim mode, agent name, output style,
 * worktree, remaining%, cumulative tokens).
 *
 * This is the scope-of-change reference for the UI revamp.
 */
import type { Meta, StoryObj } from '@storybook/react-vite'
import { MemoryRouter } from 'react-router-dom'
import { ChatContextGauge } from '../chat/ChatContextGauge'
import { ContextBar } from './ContextBar'
import { ContextGauge } from './ContextGauge'
import { CostBreakdown } from './CostBreakdown'
import { CostTooltip } from './CostTooltip'
import { ListView } from './ListView'
import { SessionBadges } from './SessionBadges'
import { SessionCard } from './SessionCard'
import type { LiveSession } from './use-live-sessions'

// ─── Mock Data Factories ───────────────────────────────────────────

const NOW = Math.floor(Date.now() / 1000)

const mockTokens = (scale = 1): TokenUsage => ({
  inputTokens: Math.round(45000 * scale),
  outputTokens: Math.round(12000 * scale),
  cacheReadTokens: Math.round(30000 * scale),
  cacheCreationTokens: Math.round(8000 * scale),
  cacheCreation5mTokens: Math.round(3000 * scale),
  cacheCreation1hrTokens: Math.round(5000 * scale),
  totalTokens: Math.round(95000 * scale),
})

const mockCost = (scale = 1): CostBreakdownType => ({
  totalUsd: 0.2847 * scale,
  inputCostUsd: 0.135 * scale,
  outputCostUsd: 0.09 * scale,
  cacheReadCostUsd: 0.03 * scale,
  cacheCreationCostUsd: 0.03 * scale,
  cacheSavingsUsd: 0.045 * scale,
  hasUnpricedUsage: false,
  unpricedInputTokens: 0,
  unpricedOutputTokens: 0,
  unpricedCacheReadTokens: 0,
  unpricedCacheCreationTokens: 0,
  pricedTokenCoverage: 1.0,
  totalCostSource: 'computed_priced_tokens_full',
})

/** Create a full LiveSession with all statusline fields populated. */
function mockSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'abc-123-def-456',
    project: 'claude-view',
    projectDisplayName: 'claude-view',
    projectPath: '/Users/dev/claude-view',
    filePath: '/Users/dev/.claude/projects/claude-view/sessions/abc-123.jsonl',
    status: 'working',
    agentState: { group: 'autonomous', state: 'acting', label: 'Working', context: null },
    gitBranch: 'feat/statusline-ui',
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: 'feat/statusline-ui',
    pid: 12345,
    title: 'Implement rate limit gauge and session badges',
    lastUserMessage: 'Add rate limit display to the session card',
    currentActivity: 'Editing ContextGauge.tsx',
    turnCount: 8,
    startedAt: NOW - 1200,
    lastActivityAt: NOW - 30,
    model: 'claude-opus-4-6-20250626',
    tokens: mockTokens(),
    contextWindowTokens: 52000,
    cost: mockCost(),
    cacheStatus: 'warm',
    currentTurnStartedAt: NOW - 45,
    lastTurnTaskSeconds: 32,
    subAgents: [
      {
        toolUseId: 'tu_001',
        agentId: 'agent-a1b2c3',
        agentType: 'Explore',
        description: 'Find statusline components',
        status: 'complete',
        startedAt: NOW - 600,
        completedAt: NOW - 580,
        durationMs: 20000,
        toolUseCount: 5,
        model: 'claude-haiku-4-5-20251001',
        inputTokens: 8000,
        outputTokens: 2000,
        cacheReadTokens: 4000,
        cacheCreationTokens: 1000,
        costUsd: 0.012,
        currentActivity: null,
      },
    ],
    teamName: null,
    editCount: 4,
    progressItems: [
      { title: 'Wire rate limit fields', status: 'completed', source: 'task' },
      {
        title: 'Add ContextGauge variant',
        status: 'in_progress',
        activeForm: 'Building gauge',
        source: 'task',
      },
      { title: 'Test dark mode', status: 'pending', source: 'task' },
    ],
    toolsUsed: [
      { name: 'context7', kind: 'mcp' },
      { name: 'commit', kind: 'skill' },
    ],
    lastCacheHitAt: NOW - 30,
    compactCount: 1,
    slug: 'statusline-ui',
    closedAt: null,
    control: null,
    hookEvents: [],

    // ─── Existing statusline fields ───
    statuslineContextWindowSize: 200_000,
    statuslineUsedPct: 26,
    statuslineCostUsd: 0.29,
    modelDisplayName: 'Opus',
    statuslineCwd: '/Users/dev/claude-view',
    statuslineProjectDir: '/Users/dev/claude-view',
    statuslineTotalDurationMs: BigInt(720000),
    statuslineApiDurationMs: BigInt(340000),
    statuslineLinesAdded: BigInt(142),
    statuslineLinesRemoved: BigInt(38),
    statuslineInputTokens: BigInt(52000),
    statuslineOutputTokens: BigInt(6800),
    statuslineCacheReadTokens: BigInt(30000),
    statuslineCacheCreationTokens: BigInt(8000),
    statuslineVersion: '1.0.33',
    exceeds200kTokens: false,
    statuslineTranscriptPath: '/Users/dev/.claude/sessions/abc-123.jsonl',

    // ─── NEW statusline fields (P1/P2/P3) ───
    statuslineOutputStyle: 'concise',
    statuslineVimMode: 'NORMAL',
    statuslineAgentName: null,
    statuslineWorktreeName: null,
    statuslineWorktreePath: null,
    statuslineWorktreeBranch: null,
    statuslineWorktreeOriginalCwd: null,
    statuslineWorktreeOriginalBranch: null,
    statuslineRemainingPct: 74,
    statuslineTotalInputTokens: 186_400,
    statuslineTotalOutputTokens: 42_300,
    statuslineRateLimit5hPct: 32,
    statuslineRateLimit5hResetsAt: NOW + 14400,
    statuslineRateLimit7dPct: 12,
    statuslineRateLimit7dResetsAt: NOW + 432000,

    phase: { current: null, labels: [], dominant: null },

    ...overrides,
  }
}

// ─── Scenario Presets ──────────────────────────────────────────────

/** Healthy session — low usage, no pressure */
const sessionHealthy = mockSession()

/** Mid-range — amber zone context, moderate rate limit */
const sessionAmber = mockSession({
  id: 'amber-session',
  title: 'Refactoring auth middleware',
  statuslineUsedPct: 67,
  statuslineRemainingPct: 33,
  contextWindowTokens: 134_000,
  turnCount: 22,
  compactCount: 2,
  statuslineRateLimit5hPct: 65,
  statuslineRateLimit5hResetsAt: NOW + 7200,
  statuslineRateLimit7dPct: 42,
  statuslineRateLimit7dResetsAt: NOW + 259200,
  statuslineTotalInputTokens: 520_000,
  statuslineTotalOutputTokens: 148_000,
  statuslineOutputStyle: 'default',
  statuslineVimMode: 'INSERT',
  cost: mockCost(3),
  tokens: mockTokens(3),
  agentState: { group: 'autonomous', state: 'acting', label: 'Working', context: null },
})

/** Danger zone — near limits everywhere */
const sessionDanger = mockSession({
  id: 'danger-session',
  title: 'Emergency prod hotfix: payment webhook timeout',
  statuslineUsedPct: 92,
  statuslineRemainingPct: 8,
  statuslineContextWindowSize: 200_000,
  contextWindowTokens: 184_000,
  turnCount: 47,
  compactCount: 6,
  statuslineRateLimit5hPct: 91,
  statuslineRateLimit5hResetsAt: NOW + 1800,
  statuslineRateLimit7dPct: 78,
  statuslineRateLimit7dResetsAt: NOW + 86400,
  statuslineTotalInputTokens: 1_420_000,
  statuslineTotalOutputTokens: 380_000,
  statuslineOutputStyle: 'concise',
  statuslineVimMode: null,
  cost: mockCost(8),
  tokens: mockTokens(8),
  agentState: { group: 'autonomous', state: 'compacting', label: 'Compacting', context: null },
  model: 'claude-sonnet-4-6-20250626',
  modelDisplayName: 'Sonnet',
  effectiveBranch: 'hotfix/payment-timeout',
})

/** Subagent active */
const sessionSubagent = mockSession({
  id: 'subagent-session',
  title: 'Building feature with parallel agents',
  statuslineAgentName: 'code-reviewer',
  statuslineVimMode: null,
  statuslineOutputStyle: 'default',
  agentState: { group: 'autonomous', state: 'acting', label: 'Reviewing code', context: null },
  teamName: 'feature-build',
  effectiveBranch: 'feat/parallel-agents',
})

/** Worktree session */
const sessionWorktree = mockSession({
  id: 'worktree-session',
  title: 'Isolated auth refactor in worktree',
  isWorktree: true,
  worktreeBranch: 'refactor/auth-v2',
  effectiveBranch: 'refactor/auth-v2',
  statuslineWorktreeName: 'auth-v2',
  statuslineWorktreePath: '/Users/dev/claude-view-worktrees/auth-v2',
  statuslineWorktreeBranch: 'refactor/auth-v2',
  statuslineWorktreeOriginalCwd: '/Users/dev/claude-view',
  statuslineWorktreeOriginalBranch: 'main',
  statuslineVimMode: 'NORMAL',
  statuslineOutputStyle: 'concise',
})

/** Needs user input */
const sessionPaused = mockSession({
  id: 'paused-session',
  title: 'Database migration — needs approval',
  status: 'paused',
  agentState: {
    group: 'needs_you',
    state: 'waiting_permission',
    label: 'Permission needed',
    context: null,
  },
  statuslineUsedPct: 45,
  statuslineRemainingPct: 55,
  statuslineRateLimit5hPct: 20,
  statuslineRateLimit5hResetsAt: NOW + 16000,
  statuslineRateLimit7dPct: 8,
  statuslineRateLimit7dResetsAt: NOW + 500000,
  currentActivity: 'Waiting for permission: DROP TABLE legacy_users',
  effectiveBranch: 'chore/db-migration',
})

/** 1M context session */
const session1M = mockSession({
  id: 'big-context-session',
  title: 'Large codebase analysis',
  statuslineContextWindowSize: 1_000_000,
  statuslineUsedPct: 38,
  statuslineRemainingPct: 62,
  contextWindowTokens: 380_000,
  statuslineTotalInputTokens: 2_800_000,
  statuslineTotalOutputTokens: 620_000,
  turnCount: 58,
  compactCount: 0,
  cost: mockCost(12),
  tokens: mockTokens(12),
  exceeds200kTokens: true,
  modelDisplayName: 'Opus',
})

const allSessions = [
  sessionHealthy,
  sessionAmber,
  sessionDanger,
  sessionSubagent,
  sessionWorktree,
  sessionPaused,
  session1M,
]

// ─── Section Component ─────────────────────────────────────────────

function Section({
  title,
  description,
  children,
}: { title: string; description: string; children: React.ReactNode }) {
  return (
    <div className="mb-10">
      <div className="mb-4 pb-2 border-b border-gray-200 dark:border-gray-800">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">{title}</h2>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">{description}</p>
      </div>
      {children}
    </div>
  )
}

function Label({ children }: { children: React.ReactNode }) {
  return <div className="text-xs font-mono text-gray-400 dark:text-gray-500 mb-1.5">{children}</div>
}

// ─── Canvas Component ──────────────────────────────────────────────

function StatuslineCanvas() {
  return (
    <Tooltip.Provider delayDuration={200}>
      <div className="w-full max-w-[1200px] mx-auto space-y-2">
        <div className="mb-8">
          <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
            Statusline UI Revamp — Component Canvas
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            All components consuming statusline data. Mock data includes all 14 new fields.
          </p>
        </div>

        {/* ─── TIER 1: Core Gauges ─── */}
        <Section
          title="Tier 1: Context Gauges"
          description="Core visualization of context window usage. New: remainingPct, totalInputTokens, totalOutputTokens."
        >
          {/* ContextGauge — compact (card mode) */}
          <div className="space-y-4">
            <div>
              <Label>ContextGauge compact — Healthy (26%)</Label>
              <div className="w-[360px]">
                <ContextGauge
                  contextWindowTokens={52000}
                  model="claude-opus-4-6-20250626"
                  group="autonomous"
                  tokens={mockTokens()}
                  turnCount={8}
                  compactCount={1}
                  statuslineContextWindowSize={200_000}
                  statuslineUsedPct={26}
                  statuslineRemainingPct={74}
                  statuslineTotalInputTokens={186_400}
                  statuslineTotalOutputTokens={42_300}
                />
              </div>
            </div>
            <div>
              <Label>ContextGauge compact — Amber (67%)</Label>
              <div className="w-[360px]">
                <ContextGauge
                  contextWindowTokens={134_000}
                  model="claude-opus-4-6-20250626"
                  group="autonomous"
                  tokens={mockTokens(3)}
                  turnCount={22}
                  compactCount={2}
                  statuslineContextWindowSize={200_000}
                  statuslineUsedPct={67}
                  statuslineRemainingPct={33}
                  statuslineTotalInputTokens={520_000}
                  statuslineTotalOutputTokens={148_000}
                />
              </div>
            </div>
            <div>
              <Label>ContextGauge compact — Danger (92%, compacting)</Label>
              <div className="w-[360px]">
                <ContextGauge
                  contextWindowTokens={184_000}
                  model="claude-sonnet-4-6-20250626"
                  group="autonomous"
                  tokens={mockTokens(8)}
                  turnCount={47}
                  compactCount={6}
                  agentStateKey="compacting"
                  agentLabel="Compacting"
                  statuslineContextWindowSize={200_000}
                  statuslineUsedPct={92}
                  statuslineRemainingPct={8}
                  statuslineTotalInputTokens={1_420_000}
                  statuslineTotalOutputTokens={380_000}
                />
              </div>
            </div>
            <div>
              <Label>ContextGauge compact — 1M context (38%)</Label>
              <div className="w-[360px]">
                <ContextGauge
                  contextWindowTokens={380_000}
                  model="claude-opus-4-6-20250626"
                  group="autonomous"
                  tokens={mockTokens(12)}
                  turnCount={58}
                  compactCount={0}
                  statuslineContextWindowSize={1_000_000}
                  statuslineUsedPct={38}
                  statuslineRemainingPct={62}
                  statuslineTotalInputTokens={2_800_000}
                  statuslineTotalOutputTokens={620_000}
                />
              </div>
            </div>

            {/* ContextGauge — expanded (detail panel mode) */}
            <div>
              <Label>ContextGauge expanded — Healthy (26%)</Label>
              <div className="w-[400px] p-4 bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-800">
                <ContextGauge
                  contextWindowTokens={52000}
                  model="claude-opus-4-6-20250626"
                  group="autonomous"
                  tokens={mockTokens()}
                  turnCount={8}
                  compactCount={1}
                  expanded
                  statuslineContextWindowSize={200_000}
                  statuslineUsedPct={26}
                  statuslineRemainingPct={74}
                  statuslineTotalInputTokens={186_400}
                  statuslineTotalOutputTokens={42_300}
                />
              </div>
            </div>
          </div>

          {/* ChatContextGauge */}
          <div className="mt-6 space-y-3">
            <Label>ChatContextGauge — variants (chat input bar gauge)</Label>
            <div className="flex items-center gap-6 flex-wrap">
              <div>
                <div className="text-[10px] text-gray-400 mb-1">26% — statusline</div>
                <ChatContextGauge percent={26} tokens={52000} limit={200_000} source="statusline" />
              </div>
              <div>
                <div className="text-[10px] text-gray-400 mb-1">67% — amber</div>
                <ChatContextGauge
                  percent={67}
                  tokens={134_000}
                  limit={200_000}
                  source="statusline"
                />
              </div>
              <div>
                <div className="text-[10px] text-gray-400 mb-1">92% — danger</div>
                <ChatContextGauge
                  percent={92}
                  tokens={184_000}
                  limit={200_000}
                  source="statusline"
                />
              </div>
              <div>
                <div className="text-[10px] text-gray-400 mb-1">38% — 1M context</div>
                <ChatContextGauge
                  percent={38}
                  tokens={380_000}
                  limit={1_000_000}
                  source="statusline"
                />
              </div>
            </div>
          </div>

          {/* ContextBar */}
          <div className="mt-6 space-y-3">
            <Label>ContextBar — inline list view gauge</Label>
            <div className="flex items-center gap-6 flex-wrap">
              <div>
                <div className="text-[10px] text-gray-400 mb-1">26%</div>
                <ContextBar percent={26} />
              </div>
              <div>
                <div className="text-[10px] text-gray-400 mb-1">67%</div>
                <ContextBar percent={67} />
              </div>
              <div>
                <div className="text-[10px] text-gray-400 mb-1">92%</div>
                <ContextBar percent={92} />
              </div>
            </div>
          </div>
        </Section>

        {/* ─── P2: Session Metadata Badges ─── */}
        <Section
          title="P2: Session Metadata Badges"
          description="Whisper-quiet pills for vim mode, agent, output style, worktree. 'default' output style is suppressed."
        >
          <div className="space-y-3">
            {[
              {
                label: 'VIM + concise',
                vim: 'NORMAL',
                agent: null,
                actx: null,
                output: 'concise',
                wt: null,
                wtPath: null,
                wtBranchName: null,
                wtOrigCwd: null,
                wtBranch: null,
              },
              {
                label: 'VIM INSERT (default suppressed)',
                vim: 'INSERT',
                agent: null,
                actx: null,
                output: 'default',
                wt: null,
                wtPath: null,
                wtBranchName: null,
                wtOrigCwd: null,
                wtBranch: null,
              },
              {
                label: 'Subagent active',
                vim: null,
                agent: 'code-reviewer',
                actx: 'Reviewing auth middleware changes',
                output: 'default',
                wt: null,
                wtPath: null,
                wtBranchName: null,
                wtOrigCwd: null,
                wtBranch: null,
              },
              {
                label: 'In worktree',
                vim: 'NORMAL',
                agent: null,
                actx: null,
                output: 'concise',
                wt: 'auth-v2',
                wtPath: '/Users/dev/claude-view-worktrees/auth-v2',
                wtBranchName: 'refactor/auth-v2',
                wtOrigCwd: '/Users/dev/claude-view',
                wtBranch: 'main',
              },
              {
                label: 'All badges',
                vim: 'NORMAL',
                agent: 'code-reviewer',
                actx: 'Analyzing diff for security issues',
                output: 'concise',
                wt: 'auth-v2',
                wtPath: '/Users/dev/claude-view-worktrees/auth-v2',
                wtBranchName: 'refactor/auth-v2',
                wtOrigCwd: '/Users/dev/claude-view',
                wtBranch: 'main',
              },
              {
                label: 'No badges (all null)',
                vim: null,
                agent: null,
                actx: null,
                output: null,
                wt: null,
                wtPath: null,
                wtBranchName: null,
                wtOrigCwd: null,
                wtBranch: null,
              },
            ].map((s) => (
              <div key={s.label} className="flex items-center gap-2 flex-wrap">
                <span className="text-xs text-gray-500 dark:text-gray-400 w-[200px] shrink-0">
                  {s.label}
                </span>
                <SessionBadges
                  vimMode={s.vim}
                  agentName={s.agent}
                  agentContext={s.actx}
                  outputStyle={s.output}
                  worktreeName={s.wt}
                  worktreePath={s.wtPath}
                  worktreeBranch={s.wtBranchName}
                  worktreeOriginalCwd={s.wtOrigCwd}
                  worktreeOriginalBranch={s.wtBranch}
                />
                {!s.vim && !s.agent && !s.output && !s.wt && (
                  <span className="text-xs text-gray-400 dark:text-gray-500 italic">
                    renders null
                  </span>
                )}
              </div>
            ))}
          </div>
        </Section>

        {/* ─── TIER 2: Session Cards ─── */}
        <Section
          title="Tier 2: Session Cards"
          description="SessionCard with all new fields. SessionBadges integrated (vim, agent, output, worktree)."
        >
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {[
              { s: sessionHealthy, label: 'Healthy (26%, low rate limit)' },
              { s: sessionAmber, label: 'Amber (67%, VIM:INSERT)' },
              { s: sessionDanger, label: 'Danger (92%, rate limit 91%)' },
              { s: sessionSubagent, label: 'Subagent: code-reviewer' },
              { s: sessionWorktree, label: 'Worktree: auth-v2' },
              { s: sessionPaused, label: 'Needs permission' },
            ].map(({ s, label }) => (
              <div key={s.id}>
                <Label>{label}</Label>
                <SessionCard session={s} currentTime={NOW} />
              </div>
            ))}
          </div>
        </Section>

        {/* ─── TIER 3: ListView ─── */}
        <Section
          title="Tier 3: ListView"
          description="Table view with context% column. Candidates for new columns: rate limit, badges."
        >
          <div className="border border-gray-200 dark:border-gray-800 rounded-lg overflow-hidden">
            <ListView sessions={allSessions} selectedId={null} onSelect={() => {}} />
          </div>
        </Section>

        {/* ─── TIER 4: Cost & Token Display ─── */}
        <Section
          title="Tier 4: Cost & Token Display"
          description="CostTooltip, CostBreakdown, ChatStatusBar. New: cumulative tokens (totalInput/Output)."
        >
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* CostBreakdown */}
            <div>
              <Label>CostBreakdown — standard session</Label>
              <div className="p-4 bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-800">
                <CostBreakdown
                  cost={mockCost()}
                  tokens={mockTokens()}
                  subAgents={sessionHealthy.subAgents}
                />
              </div>
            </div>
            <div>
              <Label>CostBreakdown — heavy session</Label>
              <div className="p-4 bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-800">
                <CostBreakdown
                  cost={mockCost(8)}
                  tokens={mockTokens(8)}
                  subAgents={sessionDanger.subAgents}
                />
              </div>
            </div>
          </div>

          {/* CostTooltip */}
          <div className="mt-4">
            <Label>CostTooltip — hover the cost badges below</Label>
            <div className="flex items-center gap-4 flex-wrap">
              {[
                { label: '$0.28', cost: mockCost(), tokens: mockTokens(), cache: 'warm' as const },
                {
                  label: '$2.28',
                  cost: mockCost(8),
                  tokens: mockTokens(8),
                  cache: 'warm' as const,
                },
              ].map((c) => (
                <CostTooltip
                  key={c.label}
                  cost={c.cost}
                  tokens={c.tokens}
                  cacheStatus={c.cache}
                  subAgents={sessionHealthy.subAgents}
                >
                  <span className="inline-flex items-center px-2 py-1 rounded text-xs font-mono bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 cursor-default">
                    {c.label}
                  </span>
                </CostTooltip>
              ))}
            </div>
          </div>
        </Section>

        {/* ─── Scope Reference ─── */}
        <Section title="Scope Reference" description="Files in scope for this revamp.">
          <div className="text-xs font-mono text-gray-500 dark:text-gray-400 space-y-0.5">
            <div className="font-semibold text-gray-700 dark:text-gray-300 mb-1">
              Tier 1 — Core Gauges
            </div>
            <div>components/live/ContextGauge.tsx (558 lines)</div>
            <div>components/chat/ChatContextGauge.tsx (127 lines)</div>
            <div>components/live/ContextBar.tsx (40 lines)</div>
            <div className="font-semibold text-gray-700 dark:text-gray-300 mt-2 mb-1">
              Tier 2 — Session Cards &amp; Detail
            </div>
            <div>components/live/SessionCard.tsx (462 lines)</div>
            <div>components/live/SessionDetailPanel.tsx (852 lines)</div>
            <div>components/live/session-panel-data.ts (247 lines)</div>
            <div className="font-semibold text-gray-700 dark:text-gray-300 mt-2 mb-1">
              Tier 3 — List &amp; Monitor Views
            </div>
            <div>components/live/ListView.tsx (268 lines)</div>
            <div>components/live/MonitorPane.tsx (422 lines)</div>
            <div>components/live/TerminalOverlay.tsx (198 lines)</div>
            <div className="font-semibold text-gray-700 dark:text-gray-300 mt-2 mb-1">
              Tier 4 — Cost &amp; Token Display
            </div>
            <div>components/live/CostTooltip.tsx (286 lines)</div>
            <div>components/live/CostBreakdown.tsx (252 lines)</div>
            <div>components/live/ChatStatusBar.tsx (60 lines)</div>
            <div className="font-semibold text-gray-700 dark:text-gray-300 mt-2 mb-1">
              Tier 5 — Hooks &amp; Data Layer
            </div>
            <div>hooks/use-context-percent.ts (40 lines)</div>
            <div>pages/ChatPageV2.tsx</div>
            <div className="mt-3 text-gray-400">
              Total: 14 files, ~3,800 lines | 14 new statusline fields
            </div>
          </div>
        </Section>
      </div>
    </Tooltip.Provider>
  )
}

// ─── Storybook Meta ────────────────────────────────────────────────

const meta = {
  title: 'Live/StatuslineCanvas',
  component: StatuslineCanvas,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <MemoryRouter>
        <Story />
      </MemoryRouter>
    ),
  ],
} satisfies Meta<typeof StatuslineCanvas>

export default meta
type Story = StoryObj<typeof meta>

export const Canvas: Story = {}
