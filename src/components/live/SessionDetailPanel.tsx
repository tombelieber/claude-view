import { useState, useEffect, useCallback, useRef } from 'react'
import { createPortal } from 'react-dom'
import { X, Terminal, Users, DollarSign, GitBranch, LayoutDashboard, Cpu, Clock, Zap, Copy, Check } from 'lucide-react'
import type { LiveSession } from './use-live-sessions'
import { RichTerminalPane } from './RichTerminalPane'
import { SwimLanes } from './SwimLanes'
import { SubAgentDrillDown } from './SubAgentDrillDown'
import { TimelineView } from './TimelineView'
import { CostBreakdown } from './CostBreakdown'
import { SubAgentPills } from './SubAgentPills'
import { ContextGauge } from './ContextGauge'
import { cn } from '../../lib/utils'
import { cleanPreviewText } from '../../utils/get-session-title'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId = 'overview' | 'terminal' | 'sub-agents' | 'cost'

interface SessionDetailPanelProps {
  session: LiveSession
  onClose: () => void
}

// ---------------------------------------------------------------------------
// Tab configuration
// ---------------------------------------------------------------------------

const TABS: { id: TabId; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { id: 'overview', label: 'Overview', icon: LayoutDashboard },
  { id: 'terminal', label: 'Terminal', icon: Terminal },
  { id: 'sub-agents', label: 'Sub-Agents', icon: Users },
  { id: 'cost', label: 'Cost', icon: DollarSign },
]

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Format token count to human-readable (e.g., 15k, 1.2M) */
function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`
  return String(n)
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
const DEFAULT_PANEL_WIDTH = 480
const MIN_PANEL_WIDTH = 320

function getStoredPanelWidth(): number {
  try {
    const stored = localStorage.getItem(PANEL_WIDTH_KEY)
    if (stored) {
      const w = parseInt(stored, 10)
      if (w >= MIN_PANEL_WIDTH && !isNaN(w)) return w
    }
  } catch { /* ignore */ }
  return DEFAULT_PANEL_WIDTH
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SessionDetailPanel({ session, onClose }: SessionDetailPanelProps) {
  const hasSubAgents = session.subAgents && session.subAgents.length > 0

  // ---- Local state ----
  const [activeTab, setActiveTab] = useState<TabId>('overview')
  const [verboseMode, setVerboseMode] = useState(false)
  const [drillDownAgent, setDrillDownAgent] = useState<{
    agentId: string; agentType: string; description: string
  } | null>(null)

  // Resizable width (persisted to localStorage)
  const [panelWidth, setPanelWidth] = useState(getStoredPanelWidth)
  const panelWidthRef = useRef(panelWidth)
  const [isResizing, setIsResizing] = useState(false)

  // Slide-in animation: mount with translate-x-full, then flip to translate-x-0
  const [isVisible, setIsVisible] = useState(false)
  useEffect(() => {
    // Trigger animation on next frame so the initial translate-x-full renders first
    const raf = requestAnimationFrame(() => setIsVisible(true))
    return () => cancelAnimationFrame(raf)
  }, [])

  // Reset tab and drill-down when session changes
  useEffect(() => {
    setActiveTab('overview')
    setDrillDownAgent(null)
  }, [session.id])

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
    navigator.clipboard.writeText(session.id).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [session.id])

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
      try { localStorage.setItem(PANEL_WIDTH_KEY, String(panelWidthRef.current)) } catch { /* ignore */ }
    }

    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }, [])

  // ---- Derived values ----
  const statusLabel = session.status === 'working' ? 'Running' : session.status === 'paused' ? 'Paused' : 'Done'
  const statusColor = session.status === 'working'
    ? 'text-green-600 dark:text-green-400'
    : session.status === 'paused'
      ? 'text-amber-600 dark:text-amber-400'
      : 'text-gray-500 dark:text-gray-400'

  // ---- Render into portal ----
  return createPortal(
    <div
      className={cn(
        'fixed top-0 right-0 h-screen z-50',
        'bg-white dark:bg-gray-950',
        'border-l border-gray-200 dark:border-gray-800',
        'shadow-2xl shadow-black/50',
        'flex flex-col',
        'transition-transform duration-200 ease-out',
        isVisible ? 'translate-x-0' : 'translate-x-full',
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
            title={session.projectPath}
          >
            {session.projectDisplayName || session.project}
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
          {session.gitBranch && (
            <span
              className="inline-flex items-center gap-1 text-[11px] font-mono text-gray-500 dark:text-gray-500 truncate max-w-[180px]"
              title={session.gitBranch}
            >
              <GitBranch className="w-3 h-3 flex-shrink-0" />
              {session.gitBranch}
            </span>
          )}

          <button
            onClick={copySessionId}
            title={`Copy session ID: ${session.id}`}
            className="inline-flex items-center gap-1 text-[11px] font-mono text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          >
            {copied ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3" />}
            {session.id.slice(0, 8)}
          </button>

          <div className="flex-1" />

          <span className="text-[11px] font-mono text-gray-500 dark:text-gray-400 tabular-nums">
            ${session.cost.totalUsd.toFixed(2)}
          </span>
          <span className="text-[11px] text-gray-400 dark:text-gray-500 tabular-nums">
            Turn {session.turnCount}
          </span>
        </div>
      </div>

      {/* ---------------------------------------------------------------- */}
      {/* Tab bar                                                         */}
      {/* ---------------------------------------------------------------- */}
      <div className="flex items-center border-b border-gray-200 dark:border-gray-800 flex-shrink-0" role="tablist">
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
              onClick={() => setVerboseMode((v) => !v)}
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
          <div className="p-4 overflow-y-auto h-full space-y-4">
            {/* Row 1: Cost + Session Info side by side */}
            <div className="grid grid-cols-2 gap-3">
              {/* Cost card (clickable -> Cost tab) */}
              <button
                onClick={() => setActiveTab('cost')}
                className="text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1.5 mb-2">
                  <DollarSign className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Cost</span>
                </div>
                <div className="text-xl font-mono font-semibold text-gray-900 dark:text-gray-100 tabular-nums">
                  ${session.cost.totalUsd.toFixed(2)}
                </div>
                <div className="flex gap-3 mt-1.5 text-[10px] font-mono text-gray-500 dark:text-gray-500 tabular-nums">
                  <span>In: ${session.cost.inputCostUsd.toFixed(2)}</span>
                  <span>Out: ${session.cost.outputCostUsd.toFixed(2)}</span>
                </div>
                {session.cost.cacheSavingsUsd > 0 && (
                  <div className="text-[10px] font-mono text-green-600 dark:text-green-400 mt-0.5">
                    Saved: ${session.cost.cacheSavingsUsd.toFixed(2)}
                  </div>
                )}
              </button>

              {/* Session info card */}
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
                    <span className="text-xs font-mono text-gray-700 dark:text-gray-300">{formatModel(session.model)}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-gray-500 dark:text-gray-500">Turns</span>
                    <span className="text-xs font-mono text-gray-700 dark:text-gray-300 tabular-nums">{session.turnCount}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-gray-500 dark:text-gray-500">Tokens</span>
                    <span className="text-xs font-mono text-gray-700 dark:text-gray-300 tabular-nums">{formatTokens(session.tokens.totalTokens)}</span>
                  </div>
                </div>
              </div>
            </div>

            {/* Context gauge */}
            <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
              <div className="flex items-center gap-1.5 mb-2">
                <Zap className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Context Window</span>
              </div>
              <ContextGauge
                contextWindowTokens={session.contextWindowTokens}
                model={session.model}
                group={session.agentState.group}
                tokens={session.tokens}
                turnCount={session.turnCount}
              />
            </div>

            {/* Sub-agents compact (clickable -> Sub-Agents tab) */}
            {hasSubAgents && (
              <button
                onClick={() => setActiveTab('sub-agents')}
                className="w-full text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1.5 mb-2">
                  <Users className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
                    Sub-Agents ({session.subAgents!.length})
                  </span>
                </div>
                <SubAgentPills subAgents={session.subAgents!} />
              </button>
            )}

            {/* Mini timeline (clickable -> Sub-Agents tab) */}
            {hasSubAgents && session.startedAt && (
              <button
                onClick={() => setActiveTab('sub-agents')}
                className="w-full text-left rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 hover:bg-gray-100 dark:hover:bg-gray-800/70 transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1.5 mb-2">
                  <Clock className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                  <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Timeline</span>
                </div>
                <TimelineView
                  subAgents={session.subAgents!}
                  sessionStartedAt={session.startedAt}
                  sessionDurationMs={
                    session.status === 'done'
                      ? (session.lastActivityAt - session.startedAt) * 1000
                      : Date.now() - session.startedAt * 1000
                  }
                />
              </button>
            )}

            {/* Last user message */}
            {session.lastUserMessage && (
              <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                <span className="text-[10px] font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">Last Prompt</span>
                <p className="text-xs text-gray-700 dark:text-gray-300 mt-1.5 line-clamp-3">{cleanPreviewText(session.lastUserMessage)}</p>
              </div>
            )}
          </div>
        )}

        {/* ---- Terminal tab ---- */}
        {activeTab === 'terminal' && (
          <RichTerminalPane
            sessionId={session.id}
            isVisible={true}
            verboseMode={verboseMode}
          />
        )}

        {/* ---- Sub-Agents tab (merged with Timeline) ---- */}
        {activeTab === 'sub-agents' && (
          <div className="flex flex-col h-full overflow-hidden">
            {drillDownAgent ? (
              <SubAgentDrillDown
                key={drillDownAgent.agentId}
                sessionId={session.id}
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
                    subAgents={session.subAgents!}
                    sessionActive={session.status === 'working'}
                    onDrillDown={handleDrillDown}
                  />
                </div>

                {/* Timeline (~50% height, only when startedAt is available) */}
                {session.startedAt && (
                  <div className="flex-1 min-h-0 overflow-y-auto p-4 border-t border-gray-200 dark:border-gray-800">
                    <TimelineView
                      subAgents={session.subAgents!}
                      sessionStartedAt={session.startedAt}
                      sessionDurationMs={
                        session.status === 'done'
                          ? (session.lastActivityAt - session.startedAt) * 1000
                          : Date.now() - session.startedAt * 1000
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
            <CostBreakdown cost={session.cost} subAgents={session.subAgents} />
          </div>
        )}
      </div>
    </div>,
    document.body,
  )
}
