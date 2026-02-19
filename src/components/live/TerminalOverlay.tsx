import { useState, useEffect, useCallback } from 'react'
import { createPortal } from 'react-dom'
import { X, GitBranch, Copy, Check } from 'lucide-react'
import type { LiveSession } from './use-live-sessions'
import { RichTerminalPane } from './RichTerminalPane'
import { ContextBar } from './ContextBar'
import { StatusDot } from './StatusDot'
import { StateBadge } from './SessionCard'
import { useMonitorStore } from '../../store/monitor-store'
import { cn } from '../../lib/utils'

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
  const toggleVerbose = useMonitorStore((s) => s.toggleVerbose)
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

  const contextPercent = Math.min(
    100,
    Math.round((session.contextWindowTokens / 200_000) * 100)
  )

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
            className="text-sm font-medium text-gray-900 dark:text-[#E6EDF3] truncate max-w-[200px]"
            title={session.projectPath}
          >
            {session.projectDisplayName || session.project}
          </span>

          {session.gitBranch && (
            <span
              className="inline-flex items-center gap-1 text-xs font-mono text-gray-400 dark:text-[#6E7681] truncate max-w-[160px]"
              title={session.gitBranch}
            >
              <GitBranch className="w-3 h-3 flex-shrink-0" />
              {session.gitBranch}
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
            ${session.cost.totalUsd.toFixed(2)}
          </span>
          <span className="text-xs text-gray-400 dark:text-[#6E7681] tabular-nums">
            Turn {session.turnCount}
          </span>
          <div className="w-16">
            <ContextBar percent={contextPercent} />
          </div>

          {/* Verbose toggle */}
          <button
            onClick={toggleVerbose}
            className={cn(
              'text-[10px] px-2 py-0.5 rounded border transition-colors',
              verboseMode
                ? 'border-blue-500 dark:border-[#1F6FEB] text-blue-600 dark:text-[#79C0FF]'
                : 'border-gray-300 dark:border-[#30363D] text-gray-400 dark:text-[#6E7681] hover:text-gray-600 dark:hover:text-[#C9D1D9]',
            )}
          >
            {verboseMode ? 'verbose' : 'compact'}
          </button>

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
          <RichTerminalPane
            sessionId={session.id}
            isVisible={true}
            verboseMode={verboseMode}
          />
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
