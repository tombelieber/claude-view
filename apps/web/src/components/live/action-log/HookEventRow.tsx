import { useState } from 'react'
import { ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../../../lib/utils'
import type { HookEventItem } from './types'

const EVENT_BADGE: Record<string, string> = {
  PreToolUse: 'Pre',
  PostToolUse: 'Post',
  PostToolUseFailure: 'Fail',
  PermissionRequest: 'Perm',
  Stop: 'Stop',
  SessionStart: 'Start',
  SessionEnd: 'End',
  UserPromptSubmit: 'Prompt',
  Notification: 'Notif',
  SubagentStart: 'Sub+',
  SubagentStop: 'Sub-',
  TeammateIdle: 'Team',
  TaskCompleted: 'Task',
  PreCompact: 'Compact',
}

function shortBadge(eventName: string): string {
  return EVENT_BADGE[eventName] ?? eventName.slice(0, 6)
}

function formatContext(ctx: string): string {
  try {
    return JSON.stringify(JSON.parse(ctx), null, 2)
  } catch {
    return ctx
  }
}

interface HookEventRowProps {
  event: HookEventItem
}

export function HookEventRow({ event }: HookEventRowProps) {
  const [expanded, setExpanded] = useState(false)
  const hasContext = !!event.context

  return (
    <div className={cn(
      'border-b border-gray-800/50',
      event.group === 'needs_you' && 'bg-amber-500/5',
    )}>
      <button
        onClick={() => hasContext && setExpanded((v) => !v)}
        className={cn(
          'w-full flex items-center gap-2 px-3 py-2 text-left transition-colors',
          hasContext && 'hover:bg-gray-800/30 cursor-pointer',
        )}
      >
        <span className="w-1.5 h-1.5 rounded-full flex-shrink-0 bg-amber-400" />

        <span className="text-[10px] font-mono px-1.5 py-0.5 rounded flex-shrink-0 min-w-[40px] text-center bg-amber-500/10 text-amber-400">
          {shortBadge(event.eventName)}
        </span>

        <span className="text-xs text-gray-300 truncate flex-1 font-mono" title={event.label}>
          {event.label}
        </span>

        {event.toolName && (
          <span className="text-[10px] font-mono text-gray-500 flex-shrink-0">
            {event.toolName}
          </span>
        )}

        {event.timestamp > 0 && (
          <span className="text-[10px] font-mono tabular-nums text-gray-600 flex-shrink-0">
            {new Date(event.timestamp * 1000).toLocaleTimeString()}
          </span>
        )}

        {hasContext && (
          expanded
            ? <ChevronDown className="w-3 h-3 text-gray-500 flex-shrink-0" />
            : <ChevronRight className="w-3 h-3 text-gray-500 flex-shrink-0" />
        )}
      </button>

      {expanded && event.context && (
        <div className="px-3 pb-3">
          <pre className="text-[10px] font-mono text-amber-300/80 bg-gray-900 rounded p-2 overflow-x-auto max-h-[200px] overflow-y-auto whitespace-pre-wrap break-all">
            {formatContext(event.context)}
          </pre>
        </div>
      )}
    </div>
  )
}
