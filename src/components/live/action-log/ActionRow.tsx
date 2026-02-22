import { useState, useCallback } from 'react'
import { ChevronRight, ChevronDown, Copy, Check } from 'lucide-react'
import { cn } from '../../../lib/utils'
import type { ActionItem } from './types'

const CATEGORY_BADGE: Record<string, string> = {
  skill: 'bg-purple-500/10 text-purple-400',
  mcp: 'bg-blue-500/10 text-blue-400',
  builtin: 'bg-gray-500/10 text-gray-400',
  agent: 'bg-indigo-500/10 text-indigo-400',
  error: 'bg-red-500/10 text-red-400',
  hook: 'bg-amber-500/10 text-amber-400',
  hook_progress: 'bg-yellow-500/10 text-yellow-400',
  system: 'bg-cyan-500/10 text-cyan-400',
  snapshot: 'bg-teal-500/10 text-teal-400',
  queue: 'bg-orange-500/10 text-orange-400',
}

const BADGE_LABELS: Record<string, string> = {
  mcp: 'MCP',
  skill: 'Skill',
  agent: 'Agent',
  error: 'Error',
  hook: 'Hook',
  hook_progress: 'Hook',
  system: 'System',
  snapshot: 'Snapshot',
  queue: 'Queue',
}

function statusDotColor(status: ActionItem['status']): string {
  switch (status) {
    case 'success': return 'bg-green-500'
    case 'error': return 'bg-red-500'
    case 'pending': return 'bg-amber-400 animate-pulse'
  }
}

function formatDuration(ms: number | undefined): { text: string; color: string } | null {
  if (ms == null) return null
  const secs = ms / 1000
  const text = secs >= 60 ? `${(secs / 60).toFixed(1)}m` : `${secs.toFixed(1)}s`
  const color = secs > 30 ? 'text-red-400' : secs > 5 ? 'text-amber-400' : 'text-gray-500 dark:text-gray-500'
  return { text, color }
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [text])

  return (
    <button
      onClick={handleCopy}
      className="text-gray-500 hover:text-gray-300 transition-colors p-0.5"
      title="Copy to clipboard"
    >
      {copied ? <Check className="w-3 h-3 text-green-400" /> : <Copy className="w-3 h-3" />}
    </button>
  )
}

function formatJson(input: string): string {
  try {
    return JSON.stringify(JSON.parse(input), null, 2)
  } catch {
    return input
  }
}

interface ActionRowProps {
  action: ActionItem
}

export function ActionRow({ action }: ActionRowProps) {
  const [expanded, setExpanded] = useState(false)
  const duration = formatDuration(action.duration)
  const badgeClass = CATEGORY_BADGE[action.category] || CATEGORY_BADGE.builtin

  const badgeLabel = action.category === 'builtin'
    ? action.toolName
    : (BADGE_LABELS[action.category] ?? action.category)

  return (
    <div
      className={cn(
        'border-b border-gray-800/50',
        action.status === 'error' && 'bg-red-500/5',
      )}
    >
      <button
        onClick={() => setExpanded((v) => !v)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-800/30 transition-colors cursor-pointer"
      >
        <span className={cn('w-1.5 h-1.5 rounded-full flex-shrink-0', statusDotColor(action.status))} />

        <span className={cn('text-[10px] font-mono px-1.5 py-0.5 rounded flex-shrink-0 min-w-[40px] text-center', badgeClass)}>
          {badgeLabel}
        </span>

        <span className="text-xs text-gray-300 truncate flex-1 font-mono" title={action.label}>
          {action.label}
        </span>

        {duration && (
          <span className={cn('text-[10px] font-mono tabular-nums flex-shrink-0', duration.color)}>
            {duration.text}
          </span>
        )}
        {action.status === 'pending' && (
          <span className="text-[10px] text-amber-400 font-mono flex-shrink-0">...</span>
        )}

        {(action.input || action.output) && (
          expanded
            ? <ChevronDown className="w-3 h-3 text-gray-500 flex-shrink-0" />
            : <ChevronRight className="w-3 h-3 text-gray-500 flex-shrink-0" />
        )}
      </button>

      {expanded && (
        <div className="px-3 pb-3 space-y-2">
          {action.timestamp && action.timestamp > 0 && (
            <div className="text-[9px] text-gray-600 font-mono mb-1">
              {new Date(action.timestamp * 1000).toLocaleTimeString()}
            </div>
          )}
          {action.input && (
            <div>
              <div className="flex items-center justify-between mb-1">
                <span className="text-[9px] font-medium text-gray-500 uppercase tracking-wider">Input</span>
                <CopyButton text={action.input} />
              </div>
              <pre className="text-[10px] font-mono text-gray-300 bg-gray-900 rounded p-2 overflow-x-auto max-h-[200px] overflow-y-auto whitespace-pre-wrap break-all">
                {formatJson(action.input)}
              </pre>
            </div>
          )}
          {action.output && (
            <div>
              <div className="flex items-center justify-between mb-1">
                <span className="text-[9px] font-medium text-gray-500 uppercase tracking-wider">Output</span>
                <CopyButton text={action.output} />
              </div>
              <pre className="text-[10px] font-mono text-gray-300 bg-gray-900 rounded p-2 overflow-x-auto max-h-[200px] overflow-y-auto whitespace-pre-wrap break-all">
                {action.output.length > 2000 ? action.output.slice(0, 2000) + '\n... (truncated)' : action.output}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
