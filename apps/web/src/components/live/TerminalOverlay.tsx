import { Check, Copy, FolderOpen, GitBranch, X } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { cn } from '../../lib/utils'
import { useMonitorStore } from '../../store/monitor-store'
import { ContextBar } from './ContextBar'
import { RichTerminalPane } from './RichTerminalPane'
import { StateBadge } from './SessionCard'
import { StatusDot } from './StatusDot'
import { ViewModeControls } from './ViewModeControls'
import { hasUnavailableCost } from './cost-display'
import { type LiveSession, sessionTotalCost } from './use-live-sessions'

interface TerminalOverlayProps {
  session: LiveSession
  onClose: () => void
}

/**
 * TerminalOverlay — near-fullscreen overlay for Monitor view.
 *
 * Opens when expanding a pane in Monitor mode. Shows the terminal feed
 * at maximum size so the user can read more output at a glance.
 */
export function TerminalOverlay({ session, onClose }: TerminalOverlayProps) {
  const [isVisible, setIsVisible] = useState(false)
  const verboseMode = useMonitorStore((s) => s.verboseMode)
  const [copied, setCopied] = useState(false)

  // Entrance animation
  useEffect(() => {
    const raf = requestAnimationFrame(() => setIsVisible(true))
    return () => cancelAnimationFrame(raf)
  }, [])

  // ESC to close
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
      }
    }
    document.addEventListener('keydown', handleKeyDown, true)
    return () => document.removeEventListener('keydown', handleKeyDown, true)
  }, [onClose])

  // Copy session ID
  const copySessionId = useCallback(() => {
    navigator.clipboard.writeText(session.id).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [session.id])

  const contextPercent = Math.min(100, Math.round((session.contextWindowTokens / 200_000) * 100))
  const totalCost = sessionTotalCost(session)
  const showUnavailableCost = hasUnavailableCost(
    totalCost,
    session.cost,
    session.tokens.totalTokens,
  )
  const formattedCost = showUnavailableCost
    ? 'Unavailable'
    : totalCost === 0
      ? '$0.00'
      : totalCost < 0.01
        ? `$${totalCost.toFixed(4)}`
        : `$${totalCost.toFixed(2)}`

  return createPortal(
    <div
      className={cn(
        'fixed inset-0 z-[60] flex items-center justify-center',
        'transition-opacity duration-200',
        isVisible ? 'opacity-100' : 'opacity-0',
      )}
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/40 dark:bg-[#010409]/80 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Panel */}
      <div
        className={cn(
          'relative flex flex-col',
          'w-[calc(100vw-32px)] h-[calc(100vh-32px)]',
          'bg-white dark:bg-[#0D1117] rounded-xl',
          'border border-gray-200 dark:border-[#30363D]',
          'shadow-2xl shadow-black/30 dark:shadow-black/80',
          'transition-transform duration-200 ease-out',
          isVisible ? 'scale-100' : 'scale-[0.97]',
        )}
      >
        {/* Header */}
        <div className="flex items-center gap-3 px-4 py-2.5 bg-gray-50 dark:bg-[#161B22] border-b border-gray-200 dark:border-[#21262D] rounded-t-xl flex-shrink-0">
          <StatusDot
            group={session.agentState.group}
            size="sm"
            pulse={session.agentState.group === 'autonomous'}
          />

          <span
            className="inline-flex items-center gap-1 text-sm font-medium text-gray-900 dark:text-[#E6EDF3] truncate max-w-50"
            title={session.projectPath}
          >
            <FolderOpen className="w-4 h-4 text-amber-500 dark:text-amber-400 shrink-0" />
            {session.projectDisplayName || session.project}
          </span>

          {session.effectiveBranch && (
            <span
              className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-mono bg-violet-50 dark:bg-violet-950/50 border border-violet-200 dark:border-violet-800 text-violet-700 dark:text-violet-300 rounded truncate max-w-40"
              title={session.effectiveBranch}
            >
              <GitBranch className="w-3 h-3 shrink-0" />
              {session.effectiveBranch}
            </span>
          )}

          <StateBadge agentState={session.agentState} />

          <button
            onClick={copySessionId}
            title={`Copy session ID: ${session.id}`}
            className="inline-flex items-center gap-1 text-[11px] font-mono text-gray-400 dark:text-[#6E7681] hover:text-gray-600 dark:hover:text-[#C9D1D9] transition-colors"
          >
            {copied ? (
              <Check className="w-3 h-3 text-green-500 dark:text-[#56D364]" />
            ) : (
              <Copy className="w-3 h-3" />
            )}
            {session.id.slice(0, 8)}
          </button>

          <div className="flex-1" />

          {/* Metrics */}
          <span className="text-xs font-mono text-gray-500 dark:text-[#8B949E] tabular-nums">
            {formattedCost}
          </span>
          <span className="text-xs text-gray-400 dark:text-[#6E7681] tabular-nums">
            Turn {session.turnCount}
          </span>
          <div className="w-16">
            <ContextBar percent={contextPercent} />
          </div>

          {/* Chat / Debug + Rich / JSON */}
          <ViewModeControls />

          {/* Close */}
          <button
            onClick={onClose}
            className="p-1 rounded text-gray-400 dark:text-[#6E7681] hover:text-gray-600 dark:hover:text-[#C9D1D9] hover:bg-gray-200 dark:hover:bg-[#30363D] transition-colors"
            aria-label="Close terminal overlay"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Terminal — takes up all remaining space (always dark) */}
        <div className="flex-1 min-h-0 overflow-hidden">
          <RichTerminalPane sessionId={session.id} isVisible={true} verboseMode={verboseMode} />
        </div>

        {/* Footer hint */}
        <div className="flex items-center px-4 py-1.5 bg-gray-50 dark:bg-[#161B22] border-t border-gray-200 dark:border-[#21262D] rounded-b-xl flex-shrink-0">
          <span className="text-[10px] text-gray-400 dark:text-[#484F58]">
            <kbd className="px-1 py-0.5 rounded bg-gray-100 dark:bg-[#21262D] text-gray-500 dark:text-[#8B949E] font-mono text-[9px] border border-gray-200 dark:border-[#30363D]">
              ESC
            </kbd>
            <span className="ml-1.5">to close</span>
          </span>
          <div className="flex-1" />
          {session.currentActivity && (
            <span className="text-[10px] text-gray-400 dark:text-[#6E7681] truncate max-w-[400px]">
              {session.currentActivity}
            </span>
          )}
        </div>
      </div>
    </div>,
    document.body,
  )
}
