import { useState, useEffect, useCallback, useRef } from 'react'
import { createPortal } from 'react-dom'
import { X, Terminal, Users, DollarSign, GitBranch, LayoutDashboard, Cpu, Clock, Zap, Copy, Check, ScrollText, Timer } from 'lucide-react'
import { type LiveSession } from './use-live-sessions'
import type { SessionPanelData } from './session-panel-data'
import { liveSessionToPanelData } from './session-panel-data'
import { SessionMetricsBar } from '../SessionMetricsBar'
import { FilesTouchedPanel, buildFilesTouched } from '../FilesTouchedPanel'
import { CommitsPanel } from '../CommitsPanel'
import { RichPane } from './RichPane'
import { useLiveSessionMessages } from '../../hooks/use-live-session-messages'
import { ActionLogTab } from './action-log'
import { SwimLanes } from './SwimLanes'
import { SubAgentDrillDown } from './SubAgentDrillDown'
import { TimelineView } from './TimelineView'
import { CostBreakdown } from './CostBreakdown'
import { SubAgentPills } from './SubAgentPills'
import { ContextGauge } from './ContextGauge'
import { CacheCountdownBar } from './CacheCountdownBar'
import { useMonitorStore } from '../../store/monitor-store'
import { cn } from '../../lib/utils'
import { formatTokenCount } from '../../lib/format-utils'
import { cleanPreviewText } from '../../utils/get-session-title'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId = 'overview' | 'terminal' | 'log' | 'sub-agents' | 'cost'

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
  { id: 'terminal', label: 'Terminal', icon: Terminal },
  { id: 'log', label: 'Log', icon: ScrollText },
  { id: 'sub-agents', label: 'Sub-Agents', icon: Users },
  { id: 'cost', label: 'Cost', icon: DollarSign },
]

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatCostUsd(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(2)}`
}

/** Format model name for display (strip long prefixes) */
function formatModel(model: string | null): string {
  if (!model) return 'unknown'
  // "claude-sonnet-4-5-20250929" -> "sonnet-4.5"
  const match = model.match(/claude-(\w+)-(\d+)(?:-(\d+))?/)
  if (match) {
    const [, name, major, minor] = match
    return minor ? `${name}-${major}.${minor}` : `${name}-${major}`
  }
  return model
}

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
      const w = parseInt(stored, 10)
      if (w >= MIN_PANEL_WIDTH && !isNaN(w)) return w
    }
  } catch { /* ignore */ }
  return fallback
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SessionDetailPanel({ session, panelData: panelDataProp, onClose, inline }: SessionDetailPanelProps) {
  // Resolve to unified data shape
  const data: SessionPanelData = panelDataProp ?? liveSessionToPanelData(session!)
  const isLive = !panelDataProp
  const hasSubAgents = data.subAgents && data.subAgents.length > 0

  // ---- Local state ----
  const [activeTab, setActiveTab] = useState<TabId>('overview')
  const verboseMode = useMonitorStore((s) => s.verboseMode)
  const toggleVerbose = useMonitorStore((s) => s.toggleVerbose)

  // Live mode: WebSocket messages; History mode: pre-loaded messages
  const { messages: liveMessages, bufferDone: liveBufferDone } = useLiveSessionMessages(
    data.id,
    isLive, // only connect WebSocket for live sessions
  )
  const richMessages = isLive ? liveMessages : (data.terminalMessages ?? [])
  const bufferDone = isLive ? liveBufferDone : true // history messages are always fully loaded

  const [drillDownAgent, setDrillDownAgent] = useState<{
    agentId: string; agentType: string; description: string
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

  // Reset tab and drill-down when session changes
  useEffect(() => {
    setActiveTab('overview')
    setDrillDownAgent(null)
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
    navigator.clipboard.writeText(data.id).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [data.id])

  // Drag-to-resize the left edge
  const handleResizeStart = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
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
      try { localStorage.setItem(inline ? INLINE_PANEL_WIDTH_KEY : PANEL_WIDTH_KEY, String(panelWidthRef.current)) } catch { /* ignore */ }
    }

    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }, [])

  // ---- Derived values ----
  const statusLabel = data.status === 'working' ? 'Running' : data.status === 'paused' ? 'Paused' : 'Done'
  const statusColor = data.status === 'working'
    ? 'text-green-600 dark:text-green-400'
    : data.status === 'paused'
      ? 'text-amber-600 dark:text-amber-400'
      : 'text-gray-500 dark:text-gray-400'
  const totalCostUsd = data.cost.totalUsd + (data.subAgents?.reduce((s, a) => s + (a.costUsd ?? 0), 0) ?? 0)
  const estimatedPrefix = data.cost?.isEstimated ? '~' : ''

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
            onClick={onClose}
            aria-label="Close detail panel"
            className="text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors p-1 flex-shrink-0"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Row 2: Metadata chips */}
        <div className="flex items-center gap-2 px-4 pb-2.5 flex-wrap">
          {data.gitBranch && (
            <span
              className="inline-flex items-center gap-1 text-[11px] font-mono text-gray-500 dark:text-gray-500 truncate max-w-[180px]"
              title={data.gitBranch}
            >
              <GitBranch className="w-3 h-3 flex-shrink-0" />
              {data.gitBranch}
            </span>
          )}

          <button
            onClick={copySessionId}
            title={`Copy session ID: ${data.id}`}
            className="inline-flex items-center gap-1 text-[11px] font-mono text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          >
            {copied ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3" />}
            {data.id.slice(0, 8)}
          </button>

          <div className="flex-1" />

          <span className="text-[11px] font-mono text-gray-500 dark:text-gray-400 tabular-nums">
            {estimatedPrefix}{formatCostUsd(totalCostUsd)}
          </span>
          <span className="text-[11px] text-gray-400 dark:text-gray-500 tabular-nums">
            Turn {data.turnCount}
          </span>
        </div>
      </div>

      {/* ---------------------------------------------------------------- */}
      {/* Tab bar                                                         */}
      {/* ---------------------------------------------------------------- */}
      <div className="flex items-center border-b border-gray-200 dark:border-gray-800 flex-shrink-0 overflow-x-auto" role="tablist">
        {TABS.map((tab) => {
          const Icon = tab.icon
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={activeTab === tab.id}
              onClick={() => { setActiveTab(tab.id); setDrillDownAgent(null) }}
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

        {/* Verbose mode toggle — only shown on Terminal tab */}
        {activeTab === 'terminal' && (
          <>
            <div className="flex-1" />
            <button
              onClick={toggleVerbose}
              className={cn(
                'text-[10px] px-1.5 py-0.5 rounded border mr-3',
                verboseMode
                  ? 'border-blue-500 text-blue-600 dark:text-blue-400'
                  : 'border-gray-300 dark:border-gray-700 text-gray-500 hover:text-gray-700 dark:hover:text-gray-400',
              )}
            >
              {verboseMode ? 'verbose' : 'compact'}
            </button>
          </>
        )}
      </div>

      {/* ---------------------------------------------------------------- */}
      {/* Tab content                                                     */}
      {/* ---------------------------------------------------------------- */}
      <div className="flex-1 min-h-0 overflow-hidden">

        {/* ---- Overview tab ---- */}
        {activeTab === 'overview' && (
          <div className="p-4 overflow-y-auto h-full">
            {/* Smart grid: small cards auto-pair at wider widths, wide cards always span full */}
            <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3">

              {/* ── Cost card (clickable -> Cost tab) ── */}
              <button
                onClick={() => setActiveTab('cost')}
                className="text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1.5 mb-2">
                  <DollarSign className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Cost</span>
                </div>
                <div className="text-xl font-mono font-semibold text-gray-900 dark:text-gray-100 tabular-nums">
                  {estimatedPrefix}{formatCostUsd(totalCostUsd)}
                </div>
                <div className="flex gap-3 mt-1.5 text-[10px] font-mono text-gray-500 dark:text-gray-500 tabular-nums">
                  <span>In: ${(data.cost?.inputCostUsd ?? 0).toFixed(2)}</span>
                  <span>Out: ${(data.cost?.outputCostUsd ?? 0).toFixed(2)}</span>
                </div>
                {(data.cost?.cacheSavingsUsd ?? 0) > 0 && (
                  <div className="text-[10px] font-mono text-green-600 dark:text-green-400 mt-0.5">
                    Saved: ${(data.cost?.cacheSavingsUsd ?? 0).toFixed(2)}
                  </div>
                )}
              </button>

              {/* ── Session info card ── */}
              <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                <div className="flex items-center gap-1.5 mb-2">
                  <Cpu className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Session</span>
                </div>
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-gray-500 dark:text-gray-500">Status</span>
                    <span className={cn('text-xs font-medium', statusColor)}>{statusLabel}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-gray-500 dark:text-gray-500">Model</span>
                    <span className="text-xs font-mono text-gray-700 dark:text-gray-300">{formatModel(data.model)}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-gray-500 dark:text-gray-500">Turns</span>
                    <span className="text-xs font-mono text-gray-700 dark:text-gray-300 tabular-nums">{data.turnCount}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-gray-500 dark:text-gray-500">Tokens</span>
                    <span className="text-xs font-mono text-gray-700 dark:text-gray-300 tabular-nums">{formatTokenCount(data.tokens.totalTokens)}</span>
                  </div>
                </div>
              </div>

              {/* ── Cache countdown (live-only) ── */}
              {isLive && (data.lastCacheHitAt || data.cacheStatus !== 'unknown') && (
                <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                  <div className="flex items-center gap-1.5 mb-2">
                    <Timer className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                    <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Prompt Cache</span>
                  </div>
                  <CacheCountdownBar
                    lastCacheHitAt={data.lastCacheHitAt ?? null}
                    cacheStatus={data.cacheStatus}
                  />
                </div>
              )}

              {/* ── Context gauge ── */}
              <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                <div className="flex items-center gap-1.5 mb-2">
                  <Zap className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Context Window</span>
                </div>
                <ContextGauge
                  contextWindowTokens={data.contextWindowTokens}
                  model={data.model}
                  group={data.agentState?.group ?? 'needs_you'}
                  tokens={data.tokens}
                  turnCount={data.turnCount}
                  expanded
                />
              </div>

              {/* ── History-only: Session Metrics ── */}
              {data.historyExtras?.sessionInfo && data.historyExtras.sessionInfo.userPromptCount > 0 && (
                <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                  <SessionMetricsBar
                    prompts={data.historyExtras.sessionInfo.userPromptCount}
                    tokens={
                      data.historyExtras.sessionInfo.totalInputTokens != null && data.historyExtras.sessionInfo.totalOutputTokens != null
                        ? BigInt(data.historyExtras.sessionInfo.totalInputTokens) + BigInt(data.historyExtras.sessionInfo.totalOutputTokens)
                        : null
                    }
                    filesRead={data.historyExtras.sessionInfo.filesReadCount}
                    filesEdited={data.historyExtras.sessionInfo.filesEditedCount}
                    reeditRate={
                      data.historyExtras.sessionInfo.filesEditedCount > 0
                        ? data.historyExtras.sessionInfo.reeditedFilesCount / data.historyExtras.sessionInfo.filesEditedCount
                        : null
                    }
                    commits={data.historyExtras.sessionInfo.commitCount}
                    variant="vertical"
                  />
                </div>
              )}

              {/* ── Sub-agents (full span — content is wide) ── */}
              {hasSubAgents && (
                <button
                  onClick={() => setActiveTab('sub-agents')}
                  className="col-[1/-1] text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
                >
                  <div className="flex items-center gap-1.5 mb-2">
                    <Users className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                    <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                      Sub-Agents ({data.subAgents!.length})
                    </span>
                  </div>
                  <SubAgentPills subAgents={data.subAgents!} />
                </button>
              )}

              {/* ── Mini timeline (full span) ── */}
              {hasSubAgents && data.startedAt && (
                <button
                  onClick={() => setActiveTab('sub-agents')}
                  className="col-[1/-1] text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
                >
                  <div className="flex items-center gap-1.5 mb-2">
                    <Clock className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                    <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Timeline</span>
                  </div>
                  <TimelineView
                    subAgents={data.subAgents!}
                    sessionStartedAt={data.startedAt}
                    sessionDurationMs={
                      data.status === 'done'
                        ? ((data.lastActivityAt ?? 0) - data.startedAt) * 1000
                        : Date.now() - data.startedAt * 1000
                    }
                  />
                </button>
              )}

              {/* ── Last user message (full span) ── */}
              {data.lastUserMessage && (
                <div className="col-[1/-1] rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Last Prompt</span>
                  <p className="text-xs text-gray-700 dark:text-gray-300 mt-1.5 line-clamp-3">{cleanPreviewText(data.lastUserMessage)}</p>
                </div>
              )}

              {/* ── History-only: Files Touched (full span) ── */}
              {data.historyExtras?.sessionDetail && (
                <div className="col-[1/-1]">
                  <FilesTouchedPanel
                    files={buildFilesTouched(
                      data.historyExtras.sessionDetail.filesRead ?? [],
                      data.historyExtras.sessionDetail.filesEdited ?? []
                    )}
                  />
                </div>
              )}

              {/* ── History-only: Linked Commits (full span) ── */}
              {data.historyExtras?.sessionDetail && (
                <div className="col-[1/-1]">
                  <CommitsPanel commits={data.historyExtras.sessionDetail.commits ?? []} />
                </div>
              )}

            </div>
          </div>
        )}

        {/* ---- Terminal tab ---- */}
        {activeTab === 'terminal' && (
          <RichPane
            messages={richMessages}
            isVisible={true}
            verboseMode={verboseMode}
            bufferDone={bufferDone}
          />
        )}

        {/* ---- Log tab ---- */}
        {activeTab === 'log' && (
          <ActionLogTab messages={richMessages} bufferDone={bufferDone} />
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
                    subAgents={data.subAgents!}
                    sessionActive={data.status === 'working'}
                    onDrillDown={handleDrillDown}
                  />
                </div>

                {/* Timeline (~50% height, only when startedAt is available) */}
                {data.startedAt && (
                  <div className="flex-1 min-h-0 overflow-y-auto p-4 border-t border-gray-200 dark:border-gray-800">
                    <TimelineView
                      subAgents={data.subAgents!}
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
                <p className="text-sm text-gray-500 dark:text-gray-400">No sub-agents in this session</p>
              </div>
            )}
          </div>
        )}

        {/* ---- Cost tab ---- */}
        {activeTab === 'cost' && (
          <div className="overflow-y-auto h-full">
            <CostBreakdown cost={data.cost} tokens={data.tokens} subAgents={data.subAgents} />
          </div>
        )}
      </div>
    </div>
  )

  return inline ? content : createPortal(content, document.body)
}
