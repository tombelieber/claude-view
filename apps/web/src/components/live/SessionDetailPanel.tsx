import type { InteractionBlock } from '@claude-view/shared/types/blocks'
import type { PermissionRequest } from '@claude-view/shared/types/sidecar-protocol'
import {
  Check,
  CheckSquare,
  Clock,
  Copy,
  DollarSign,
  FileDiff,
  FileText,
  GitBranch,
  LayoutDashboard,
  MessageSquare,
  ScrollText,
  Terminal,
  Timer,
  TreePine,
  Users,
  UsersRound,
  X,
  Zap,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useSearchParams } from 'react-router-dom'
import { ConversationActionsProvider } from '../../contexts/conversation-actions-context'
import { useConversation } from '../../hooks/use-conversation'
import { useFileHistory } from '../../hooks/use-file-history'
import { useHookEvents } from '../../hooks/use-hook-events'
import { useLiveSessionMessages } from '../../hooks/use-live-session-messages'
import { useModelOptions } from '../../hooks/use-models'
import { usePlanDocuments } from '../../hooks/use-plan-documents'
import { useSessionCapabilities } from '../../hooks/use-session-capabilities'
import { useSessionDetail } from '../../hooks/use-session-detail'
import { computeCategoryCounts } from '../../lib/compute-category-counts'
import { deriveInputBarState } from '../../lib/control-status-map'
import { formatModelName } from '../../lib/format-model'
import { formatCostUsd } from '../../lib/format-utils'
import { getContextLimit } from '../../lib/model-context-windows'
import { cn } from '../../lib/utils'
import { useMonitorStore } from '../../store/monitor-store'
import { COST_CATEGORY_COLORS } from '../../theme'
import { cleanPreviewText } from '../../utils/get-session-title'
import { CommitsPanel } from '../CommitsPanel'
import { FilesTouchedPanel, buildFilesTouched } from '../FilesTouchedPanel'
import { SessionMetricsBar } from '../SessionMetricsBar'
import { ChatInputBar } from '../chat/ChatInputBar'
import { PermissionCard } from '../chat/cards/PermissionCard'
import { TeamsTab } from '../teams/TeamsTab'
import { CacheCountdownBar } from './CacheCountdownBar'
import { ChangesTab } from './ChangesTab'
import { CompactChatTab } from './CompactChatTab'
import { ContextGauge } from './ContextGauge'
import { CostBreakdown } from './CostBreakdown'
import { PlanTab } from './PlanTab'
import { RichPane } from './RichPane'
import { SubAgentDrillDown } from './SubAgentDrillDown'
import { SubAgentPills } from './SubAgentPills'
import { SwimLanes } from './SwimLanes'
import { TaskDetailTab } from './TaskDetailTab'
import { TasksOverviewSection } from './TasksOverviewSection'
import { TimelineView } from './TimelineView'
import { ViewModeControls } from './ViewModeControls'
import { ActionLogTab } from './action-log'
import { hasUnavailableCost } from './cost-display'
import { getEffectiveBranch } from './effective-branch'
import type { SessionPanelData } from './session-panel-data'
import { liveSessionToPanelData } from './session-panel-data'
import type { LiveSession } from './use-live-sessions'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId =
  | 'overview'
  | 'chat'
  | 'terminal'
  | 'log'
  | 'sub-agents'
  | 'teams'
  | 'cost'
  | 'tasks'
  | 'changes'
  | 'plan'

interface SessionDetailPanelProps {
  /** Live session (existing callers) */
  session?: LiveSession
  /** Unified panel data (new — for history detail) */
  panelData?: SessionPanelData
  onClose: () => void
  /** When true, render inline as a flex child instead of a fixed portal overlay. */
  inline?: boolean
}

// ---------------------------------------------------------------------------
// Tab configuration
// ---------------------------------------------------------------------------

const TABS: { id: TabId; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { id: 'overview', label: 'Overview', icon: LayoutDashboard },
  { id: 'chat', label: 'Chat', icon: MessageSquare },
  { id: 'terminal', label: 'Terminal', icon: Terminal },
  { id: 'log', label: 'Log', icon: ScrollText },
  { id: 'sub-agents', label: 'Sub-Agents', icon: Users },
  { id: 'teams', label: 'Teams', icon: UsersRound },
  { id: 'cost', label: 'Cost', icon: DollarSign },
]

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Resize persistence
// ---------------------------------------------------------------------------

const PANEL_WIDTH_KEY = 'mc-panel-width'
const INLINE_PANEL_WIDTH_KEY = 'mc-inline-panel-width'
const DEFAULT_PANEL_WIDTH = 480
const DEFAULT_INLINE_PANEL_WIDTH = 288 // w-72 — matches left sidenav
const MIN_PANEL_WIDTH = 288

function getStoredPanelWidth(isInline: boolean): number {
  const key = isInline ? INLINE_PANEL_WIDTH_KEY : PANEL_WIDTH_KEY
  const fallback = isInline ? DEFAULT_INLINE_PANEL_WIDTH : DEFAULT_PANEL_WIDTH
  try {
    const stored = localStorage.getItem(key)
    if (stored) {
      const w = Number.parseInt(stored, 10)
      if (w >= MIN_PANEL_WIDTH && !Number.isNaN(w)) return w
    }
  } catch (e) {
    console.debug('[SessionDetailPanel] localStorage access failed:', e)
  }
  return fallback
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SessionDetailPanel({
  session,
  panelData: panelDataProp,
  onClose,
  inline,
}: SessionDetailPanelProps) {
  // Resolve to unified data shape
  // Callers always provide either panelData (history) or session (live) — never neither.
  // biome-ignore lint/style/noNonNullAssertion: callers guarantee panelDataProp or session is always set
  const data: SessionPanelData = panelDataProp ?? liveSessionToPanelData(session!)
  const isLive = !panelDataProp
  const hasSubAgents = data.subAgents && data.subAgents.length > 0

  // Live sessions: use SSE-driven version counters
  // For history sessions, version is undefined → query key is stable → no unnecessary refetches
  const editVersion = isLive ? session?.editCount : undefined
  const taskVersion = isLive ? (session?.progressItems?.length ?? 0) : undefined

  // For live sessions, fetch tasks from API (the SSE live stream does not include persistent task data)
  const { data: liveSessionDetail } = useSessionDetail(isLive ? data.id : null, taskVersion)
  const tasks = isLive ? liveSessionDetail?.tasks : data.tasks
  const hasTasks = tasks && tasks.length > 0

  // File history (fetched on demand for all sessions)
  const { data: fileHistory } = useFileHistory(data.id, editVersion)
  const hasChanges = fileHistory && fileHistory.files.length > 0

  // Plan documents — hook takes sessionId (endpoint resolves slug server-side)
  const { data: planDocuments } = usePlanDocuments(
    data.id,
    data.hasPlans || !!data.slug,
    editVersion,
  )
  const hasPlans = planDocuments && planDocuments.length > 0

  // ---- Unified conversation hook — handles WS lifecycle + blocks + actions ----
  const {
    blocks: convBlocks,
    actions: convActions,
    sessionInfo: convInfo,
  } = useConversation(data.id)

  // Command palette capabilities
  const sdpCapabilities = useSessionCapabilities(convInfo)
  const { options: sdpModelOptions } = useModelOptions()

  // Build ControlCallbacks from convActions for RichPane interactive cards
  const controlCallbacks = useMemo(
    () => ({
      answerQuestion: convActions.answerQuestion,
      respondPermission: (requestId: string, allowed: boolean) =>
        convActions.respondPermission(requestId, allowed),
      approvePlan: convActions.approvePlan,
      submitElicitation: convActions.submitElicitation,
    }),
    [convActions],
  )

  // Find the first unresolved permission request from the live block stream
  const pendingPermission = useMemo(
    () =>
      convBlocks.find(
        (b) => b.type === 'interaction' && b.variant === 'permission' && !b.resolved,
      ) as InteractionBlock | undefined,
    [convBlocks],
  )

  // ---- Teams tab (conditional — only show when session is a team lead) ----
  const hasTeam = !!data.teamName

  // ---- URL param: ?tab=teams ----
  const [searchParams] = useSearchParams()
  const initialTab = searchParams.get('tab') as TabId | null

  // ---- Local state ----
  const [activeTab, setActiveTab] = useState<TabId>(initialTab ?? 'overview')
  const verboseMode = useMonitorStore((s) => s.verboseMode)

  // Live mode: WebSocket messages; History mode: pre-loaded messages
  const {
    messages: liveMessages,
    hookEvents: _liveHookEvents,
    bufferDone: liveBufferDone,
  } = useLiveSessionMessages(
    data.id,
    isLive, // only connect WebSocket for live sessions
  )
  const richMessages = isLive ? liveMessages : (data.terminalMessages ?? [])
  const bufferDone = isLive ? liveBufferDone : true // history messages are always fully loaded

  // Shared category counts — computed once, passed to both Terminal and Log tabs
  const categoryCounts = useMemo(() => computeCategoryCounts(richMessages), [richMessages])

  // Historical hook events (REST fetch for non-live sessions)
  useHookEvents(data.id, !isLive)

  const [drillDownAgent, setDrillDownAgent] = useState<{
    agentId: string
    agentType: string
    description: string
  } | null>(null)

  // Resizable width (persisted to localStorage)
  const [panelWidth, setPanelWidth] = useState(() => getStoredPanelWidth(!!inline))
  const panelWidthRef = useRef(panelWidth)
  const [isResizing, setIsResizing] = useState(false)

  // Slide-in animation: mount with translate-x-full, then flip to translate-x-0
  // Skipped in inline mode (already visible in the flex layout)
  const [isVisible, setIsVisible] = useState(!!inline)
  useEffect(() => {
    if (inline) return
    // Trigger animation on next frame so the initial translate-x-full renders first
    const raf = requestAnimationFrame(() => setIsVisible(true))
    return () => cancelAnimationFrame(raf)
  }, [inline])

  // Reset tab and drill-down when session changes (skip initial mount to preserve ?tab= URL param)
  const prevDataIdRef = useRef(data.id)
  useEffect(() => {
    if (prevDataIdRef.current !== data.id) {
      setActiveTab('overview')
      setDrillDownAgent(null)
      prevDataIdRef.current = data.id
    }
  }, [data.id])

  // ESC key handling: drill-down first, then close
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (drillDownAgent) {
          setDrillDownAgent(null)
        } else {
          onClose()
        }
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [onClose, drillDownAgent])

  const handleDrillDown = useCallback((agentId: string, agentType: string, description: string) => {
    setDrillDownAgent({ agentId, agentType, description })
  }, [])

  // Copy session ID to clipboard
  const [copied, setCopied] = useState(false)
  const copySessionId = useCallback(() => {
    navigator.clipboard.writeText(data.id).then(
      () => {
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      },
      (err) => {
        console.debug('[SessionDetailPanel] Clipboard write failed:', err)
      },
    )
  }, [data.id])

  // Drag-to-resize the left edge
  const handleResizeStart = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault()
      setIsResizing(true)
      const startX = e.clientX
      const startW = panelWidthRef.current

      const onMove = (ev: PointerEvent) => {
        const delta = startX - ev.clientX
        const maxWidth = window.innerWidth * 0.9
        const newWidth = Math.round(Math.min(maxWidth, Math.max(MIN_PANEL_WIDTH, startW + delta)))
        panelWidthRef.current = newWidth
        setPanelWidth(newWidth)
      }

      const onUp = () => {
        setIsResizing(false)
        window.removeEventListener('pointermove', onMove)
        window.removeEventListener('pointerup', onUp)
        try {
          localStorage.setItem(
            inline ? INLINE_PANEL_WIDTH_KEY : PANEL_WIDTH_KEY,
            String(panelWidthRef.current),
          )
        } catch (e) {
          console.debug('[SessionDetailPanel] localStorage access failed:', e)
        }
      }

      window.addEventListener('pointermove', onMove)
      window.addEventListener('pointerup', onUp)
    },
    [inline],
  )

  // ---- Derived values ----
  const statusLabel =
    data.status === 'working' ? 'Running' : data.status === 'paused' ? 'Paused' : 'Done'
  const statusColor =
    data.status === 'working'
      ? 'text-green-600 dark:text-green-400'
      : data.status === 'paused'
        ? 'text-amber-600 dark:text-amber-400'
        : 'text-gray-500 dark:text-gray-400'
  // Canonical display rule:
  // - If rich/live-calculated cost exists, use it (matches live parser math).
  // - Fall back to DB total_cost_usd only when rich data is unavailable.
  const dbCostUsd = data.historyExtras?.sessionInfo?.totalCostUsd
  const subAgentCostUsd = data.subAgents?.reduce((s, a) => s + (a.costUsd ?? 0), 0) ?? 0
  const calculatedCostUsd = data.cost.totalUsd + subAgentCostUsd
  const totalCostUsd = calculatedCostUsd > 0 ? calculatedCostUsd : (dbCostUsd ?? 0)
  const totalCostLabel = hasUnavailableCost(totalCostUsd, data.cost, data.tokens.totalTokens)
    ? 'Unavailable'
    : formatCostUsd(totalCostUsd)
  const cacheCreation5mTokens = data.tokens?.cacheCreation5mTokens ?? 0
  const cacheCreation1hrTokens = data.tokens?.cacheCreation1hrTokens ?? 0
  const hasCacheCreationSplit = cacheCreation5mTokens > 0 || cacheCreation1hrTokens > 0

  // ---- Render ----
  const content = (
    <div
      className={cn(
        inline
          ? 'relative h-full flex-shrink-0 overflow-hidden border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900'
          : cn(
              'fixed top-0 right-0 h-screen z-50',
              'bg-white dark:bg-gray-950',
              'border-l border-gray-200 dark:border-gray-800',
              'shadow-2xl shadow-black/50',
              'transition-transform duration-200 ease-out',
              isVisible ? 'translate-x-0' : 'translate-x-full',
            ),
        'flex flex-col',
        isResizing && 'select-none',
      )}
      style={{ width: panelWidth }}
    >
      {/* Resize handle (left edge) */}
      <div
        onPointerDown={handleResizeStart}
        className="absolute top-0 left-0 w-1.5 h-full cursor-col-resize z-10 group"
      >
        <div className="w-px h-full mx-auto bg-transparent group-hover:bg-indigo-500/40 group-active:bg-indigo-500/60 transition-colors" />
      </div>

      {/* ---------------------------------------------------------------- */}
      {/* Header                                                          */}
      {/* ---------------------------------------------------------------- */}
      <div className="border-b border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 flex-shrink-0">
        {/* Row 1: Project name + close */}
        <div className="flex items-center gap-2 px-4 pt-3 pb-1">
          <span
            className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate flex-1"
            title={data.projectPath}
          >
            {data.projectDisplayName || data.project}
          </span>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close detail panel"
            className="text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors p-1 flex-shrink-0"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Row 2: Metadata chips */}
        <div className="flex items-center gap-2 px-4 pb-2.5 flex-wrap">
          {(() => {
            const { branch, driftOrigin, isWorktree } = getEffectiveBranch(
              data.gitBranch,
              data.worktreeBranch ?? null,
              data.isWorktree ?? false,
            )
            if (!branch) return null
            return (
              <span
                className="inline-flex items-center gap-1 text-[11px] font-mono text-gray-500 dark:text-gray-500"
                title={branch}
              >
                <GitBranch className="w-3 h-3 flex-shrink-0" />
                {branch}
                {isWorktree && (
                  <TreePine className="w-3 h-3 flex-shrink-0 text-green-600 dark:text-green-400" />
                )}
                {driftOrigin && (
                  <span className="text-[10px] text-gray-400 dark:text-gray-500 ml-0.5">
                    {'↗'}
                    {driftOrigin}
                  </span>
                )}
              </span>
            )
          })()}

          <button
            type="button"
            onClick={copySessionId}
            title={`Copy session ID: ${data.id}`}
            className="inline-flex items-center gap-1 text-[11px] font-mono text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          >
            {copied ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3" />}
            {data.id.slice(0, 8)}
          </button>

          <div className="flex-1" />

          <span className="text-[11px] font-mono text-gray-500 dark:text-gray-400 tabular-nums">
            {totalCostLabel}
          </span>
          <span className="text-[11px] text-gray-400 dark:text-gray-500 tabular-nums">
            Turn {data.turnCount}
          </span>
        </div>
      </div>

      {/* ---------------------------------------------------------------- */}
      {/* Tab bar                                                         */}
      {/* ---------------------------------------------------------------- */}
      <div
        className="flex items-center border-b border-gray-200 dark:border-gray-800 flex-shrink-0 overflow-x-auto"
        role="tablist"
      >
        {TABS.filter((tab) => tab.id !== 'teams' || hasTeam).map((tab) => {
          const Icon = tab.icon
          return (
            <button
              type="button"
              key={tab.id}
              role="tab"
              aria-selected={activeTab === tab.id}
              onClick={() => {
                setActiveTab(tab.id)
                setDrillDownAgent(null)
              }}
              className={cn(
                'flex items-center gap-1.5 px-3 py-2.5 text-xs font-medium transition-colors border-b-2',
                activeTab === tab.id
                  ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400'
                  : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-400',
              )}
            >
              <Icon className="w-3.5 h-3.5" />
              {tab.label}
            </button>
          )
        })}

        {/* Conditional tabs — only shown when data exists */}
        {hasTasks && (
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'tasks'}
            onClick={() => {
              setActiveTab('tasks')
              setDrillDownAgent(null)
            }}
            className={cn(
              'flex items-center gap-1.5 px-3 py-2.5 text-xs font-medium transition-colors border-b-2',
              activeTab === 'tasks'
                ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-400',
            )}
          >
            <CheckSquare className="w-3.5 h-3.5" />
            Tasks
          </button>
        )}
        {hasChanges && (
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'changes'}
            onClick={() => {
              setActiveTab('changes')
              setDrillDownAgent(null)
            }}
            className={cn(
              'flex items-center gap-1.5 px-3 py-2.5 text-xs font-medium transition-colors border-b-2',
              activeTab === 'changes'
                ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-400',
            )}
          >
            <FileDiff className="w-3.5 h-3.5" />
            Changes
          </button>
        )}
        {hasPlans && (
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === 'plan'}
            onClick={() => {
              setActiveTab('plan')
              setDrillDownAgent(null)
            }}
            className={cn(
              'flex items-center gap-1.5 px-3 py-2.5 text-xs font-medium transition-colors border-b-2',
              activeTab === 'plan'
                ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-400',
            )}
          >
            <FileText className="w-3.5 h-3.5" />
            Plan
          </button>
        )}

        {/* Chat / Debug + Rich / JSON — only shown on Terminal tab */}
        {activeTab === 'terminal' && (
          <>
            <div className="flex-1" />
            <ViewModeControls className="mr-2" />
          </>
        )}
      </div>

      {/* ---------------------------------------------------------------- */}
      {/* Tab content                                                     */}
      {/* ---------------------------------------------------------------- */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {/* ---- Overview tab ---- */}
        {activeTab === 'overview' && (
          <div className="p-4 overflow-y-auto h-full space-y-3">
            {/* ── Quick status (lightweight metadata → inline strip, not card-worthy) ── */}
            <div className="flex items-center gap-2 text-xs">
              <span className={cn('inline-flex items-center gap-1.5 font-medium', statusColor)}>
                <span
                  className={cn(
                    'w-1.5 h-1.5 rounded-full flex-shrink-0',
                    data.status === 'working' && 'bg-green-500 animate-pulse',
                    data.status === 'paused' && 'bg-amber-500',
                    data.status !== 'working' &&
                      data.status !== 'paused' &&
                      'bg-gray-400 dark:bg-gray-500',
                  )}
                />
                {statusLabel}
              </span>
              <span className="text-gray-300 dark:text-gray-600 select-none" aria-hidden>
                ·
              </span>
              <span className="font-mono text-gray-600 dark:text-gray-400">
                {data.modelDisplayName ??
                  (data.model ? formatModelName(data.model) : null) ??
                  'unknown'}
              </span>
            </div>

            {/* ── Tasks (first section — primary monitoring concern) ── */}
            {data.progressItems && data.progressItems.length > 0 && (
              <TasksOverviewSection items={data.progressItems} />
            )}

            {/* ── Cost + Cache (similar density — pair at wider widths, stack at narrow) ── */}
            <div className="grid grid-cols-[repeat(auto-fit,minmax(200px,1fr))] gap-3">
              {/* Cost card (clickable → Cost tab) */}
              <button
                type="button"
                onClick={() => setActiveTab('cost')}
                className="text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1.5 mb-2">
                  <DollarSign className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                    Cost
                  </span>
                </div>
                <div className="text-xl font-mono font-semibold text-gray-900 dark:text-gray-100 tabular-nums">
                  {totalCostLabel}
                </div>
                <div className="flex flex-wrap gap-x-3 gap-y-0.5 mt-1.5 text-[10px] font-mono text-gray-500 dark:text-gray-500 tabular-nums">
                  <span>In: ${(data.cost?.inputCostUsd ?? 0).toFixed(2)}</span>
                  <span>Out: ${(data.cost?.outputCostUsd ?? 0).toFixed(2)}</span>
                  <span>CacheR: ${(data.cost?.cacheReadCostUsd ?? 0).toFixed(2)}</span>
                  <span>CacheW: ${(data.cost?.cacheCreationCostUsd ?? 0).toFixed(2)}</span>
                </div>
                {hasSubAgents && (
                  <div className="mt-0.5 text-[10px] font-mono text-gray-400 dark:text-gray-500 tabular-nums">
                    + {data.subAgents?.length ?? 0} sub-agent
                    {(data.subAgents?.length ?? 0) !== 1 ? 's' : ''}: ${subAgentCostUsd.toFixed(2)}
                  </div>
                )}
                {hasCacheCreationSplit && (
                  <div className="mt-0.5 text-[10px] font-mono text-gray-400 dark:text-gray-500 tabular-nums">
                    CacheW tokens: 5m {cacheCreation5mTokens.toLocaleString()} · 1h{' '}
                    {cacheCreation1hrTokens.toLocaleString()}
                  </div>
                )}
                {(data.cost?.cacheSavingsUsd ?? 0) > 0 && (
                  <div
                    className={`text-[10px] font-mono ${COST_CATEGORY_COLORS.savings.text} mt-0.5`}
                  >
                    Saved: ${(data.cost?.cacheSavingsUsd ?? 0).toFixed(2)}
                  </div>
                )}
              </button>

              {/* Cache countdown (live-only — similar weight to cost, pairs well) */}
              {isLive && (data.lastCacheHitAt || data.cacheStatus !== 'unknown') && (
                <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                  <div className="flex items-center gap-1.5 mb-2">
                    <Timer className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                    <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                      Prompt Cache
                    </span>
                  </div>
                  <CacheCountdownBar
                    lastCacheHitAt={data.lastCacheHitAt ?? null}
                    cacheStatus={data.cacheStatus}
                  />
                </div>
              )}
            </div>

            {/* ── Context window (dense content — always full width) ── */}
            <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
              <div className="flex items-center gap-1.5 mb-2">
                <Zap className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                  Context Window
                </span>
              </div>
              <ContextGauge
                contextWindowTokens={data.contextWindowTokens}
                model={data.model}
                group={data.agentState?.group ?? 'needs_you'}
                tokens={data.tokens}
                turnCount={data.turnCount}
                compactCount={data.compactCount}
                statuslineContextWindowSize={data.statuslineContextWindowSize}
                statuslineUsedPct={data.statuslineUsedPct}
                expanded
              />
            </div>

            {/* ── Session metrics (history-only) ── */}
            {data.historyExtras?.sessionInfo &&
              data.historyExtras.sessionInfo.userPromptCount > 0 && (
                <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                  <SessionMetricsBar
                    prompts={data.historyExtras.sessionInfo.userPromptCount}
                    tokens={
                      data.historyExtras.sessionInfo.totalInputTokens != null &&
                      data.historyExtras.sessionInfo.totalOutputTokens != null
                        ? BigInt(data.historyExtras.sessionInfo.totalInputTokens) +
                          BigInt(data.historyExtras.sessionInfo.totalOutputTokens)
                        : null
                    }
                    filesRead={data.historyExtras.sessionInfo.filesReadCount}
                    filesEdited={data.historyExtras.sessionInfo.filesEditedCount}
                    reeditRate={
                      data.historyExtras.sessionInfo.filesEditedCount > 0
                        ? data.historyExtras.sessionInfo.reeditedFilesCount /
                          data.historyExtras.sessionInfo.filesEditedCount
                        : null
                    }
                    commits={data.historyExtras.sessionInfo.commitCount}
                    variant="vertical"
                  />
                </div>
              )}

            {/* ── Sub-agents (clickable → Sub-Agents tab) ── */}
            {hasSubAgents && (
              <button
                type="button"
                onClick={() => setActiveTab('sub-agents')}
                className="w-full text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1.5 mb-2">
                  <Users className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                    Sub-Agents ({(data.subAgents ?? []).length})
                  </span>
                </div>
                <SubAgentPills subAgents={data.subAgents ?? []} />
              </button>
            )}

            {/* ── Mini timeline (clickable → Sub-Agents tab) ── */}
            {hasSubAgents && data.startedAt && (
              <button
                type="button"
                onClick={() => setActiveTab('sub-agents')}
                className="w-full text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1.5 mb-2">
                  <Clock className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                    Timeline
                  </span>
                </div>
                <TimelineView
                  subAgents={data.subAgents ?? []}
                  sessionStartedAt={data.startedAt}
                  sessionDurationMs={
                    data.status === 'done'
                      ? ((data.lastActivityAt ?? 0) - data.startedAt) * 1000
                      : Date.now() - data.startedAt * 1000
                  }
                />
              </button>
            )}

            {/* ── Last user message ── */}
            {data.lastUserMessage && (
              <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                  Last Prompt
                </span>
                <p className="text-xs text-gray-700 dark:text-gray-300 mt-1.5 line-clamp-3">
                  {cleanPreviewText(data.lastUserMessage)}
                </p>
              </div>
            )}

            {/* ── Files Touched (history-only) ── */}
            {data.historyExtras?.sessionDetail && (
              <FilesTouchedPanel
                files={buildFilesTouched(
                  data.historyExtras.sessionDetail.filesRead ?? [],
                  data.historyExtras.sessionDetail.filesEdited ?? [],
                )}
              />
            )}

            {/* ── Commits (history-only) ── */}
            {data.historyExtras?.sessionDetail && (
              <CommitsPanel commits={data.historyExtras.sessionDetail.commits ?? []} />
            )}
          </div>
        )}

        {/* ---- Chat tab (compact block renderer) ---- */}
        {activeTab === 'chat' && (
          <div className="flex flex-col h-full overflow-hidden">
            <ConversationActionsProvider
              actions={{
                retryMessage: convActions.retryMessage,
                respondPermission: convActions.respondPermission,
                answerQuestion: convActions.answerQuestion,
                approvePlan: convActions.approvePlan,
                submitElicitation: convActions.submitElicitation,
              }}
            >
              <CompactChatTab blocks={convBlocks} />
            </ConversationActionsProvider>
          </div>
        )}

        {/* ---- Terminal tab ---- */}
        {activeTab === 'terminal' && (
          <div className="flex flex-col h-full">
            <div className="flex-1 min-h-0 overflow-hidden">
              <RichPane
                messages={richMessages}
                isVisible={true}
                verboseMode={verboseMode}
                bufferDone={bufferDone}
                categoryCounts={categoryCounts}
                controlCallbacks={controlCallbacks}
              />
            </div>
            {pendingPermission && pendingPermission.variant === 'permission' && (
              <PermissionCard
                permission={pendingPermission.data as PermissionRequest}
                onRespond={convActions.respondPermission}
              />
            )}
            {(convInfo.isLive || convInfo.canResumeLazy) && (
              <ChatInputBar
                onSend={convActions.sendMessage}
                state={deriveInputBarState(
                  convInfo.sessionState,
                  convInfo.isLive,
                  convInfo.canResumeLazy,
                )}
                contextPercent={
                  data.contextWindowTokens > 0
                    ? Math.min(
                        100,
                        Math.round(
                          data.statuslineUsedPct ??
                            (data.contextWindowTokens /
                              getContextLimit(
                                data.model,
                                data.contextWindowTokens,
                                data.statuslineContextWindowSize,
                              )) *
                              100,
                        ),
                      )
                    : undefined
                }
                capabilities={sdpCapabilities}
                modelOptions={sdpModelOptions}
                onCommand={(cmd) => convActions.sendMessage(`/${cmd}`)}
                onAgent={(agent) => convActions.sendMessage(`@${agent}`)}
              />
            )}
          </div>
        )}

        {/* ---- Log tab ---- */}
        {activeTab === 'log' && (
          <ActionLogTab
            messages={richMessages}
            bufferDone={bufferDone}
            categoryCounts={categoryCounts}
          />
        )}

        {/* ---- Sub-Agents tab (merged with Timeline) ---- */}
        {activeTab === 'sub-agents' && (
          <div className="flex flex-col h-full overflow-hidden">
            {drillDownAgent ? (
              <SubAgentDrillDown
                key={drillDownAgent.agentId}
                sessionId={data.id}
                agentId={drillDownAgent.agentId}
                agentType={drillDownAgent.agentType}
                description={drillDownAgent.description}
                onClose={() => setDrillDownAgent(null)}
              />
            ) : hasSubAgents ? (
              <>
                {/* Swim lanes (~50% height) */}
                <div className="flex-1 min-h-0 overflow-y-auto p-4">
                  <SwimLanes
                    subAgents={data.subAgents ?? []}
                    sessionActive={data.status === 'working'}
                    onDrillDown={handleDrillDown}
                  />
                </div>

                {/* Timeline (~50% height, only when startedAt is available) */}
                {data.startedAt && (
                  <div className="flex-1 min-h-0 overflow-y-auto p-4 border-t border-gray-200 dark:border-gray-800">
                    <TimelineView
                      subAgents={data.subAgents ?? []}
                      sessionStartedAt={data.startedAt}
                      sessionDurationMs={
                        data.status === 'done'
                          ? ((data.lastActivityAt ?? 0) - data.startedAt) * 1000
                          : Date.now() - data.startedAt * 1000
                      }
                    />
                  </div>
                )}
              </>
            ) : (
              <div className="flex items-center justify-center h-full">
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  No sub-agents in this session
                </p>
              </div>
            )}
          </div>
        )}

        {/* ---- Teams tab ---- */}
        {activeTab === 'teams' && data.teamName && (
          <TeamsTab teamName={data.teamName} inboxVersion={session?.teamInboxCount} />
        )}

        {/* ---- Cost tab ---- */}
        {activeTab === 'cost' && (
          <div className="overflow-y-auto h-full">
            <CostBreakdown cost={data.cost} tokens={data.tokens} subAgents={data.subAgents} />
          </div>
        )}

        {/* ---- Tasks tab ---- */}
        {activeTab === 'tasks' && hasTasks && tasks && <TaskDetailTab tasks={tasks} />}

        {/* ---- Changes tab ---- */}
        {activeTab === 'changes' && hasChanges && fileHistory && (
          <ChangesTab
            fileHistory={fileHistory}
            sessionId={data.id}
            projectPath={data.projectPath}
          />
        )}

        {/* ---- Plan tab ---- */}
        {activeTab === 'plan' && hasPlans && planDocuments && <PlanTab plans={planDocuments} />}
      </div>
    </div>
  )

  return inline ? content : createPortal(content, document.body)
}
