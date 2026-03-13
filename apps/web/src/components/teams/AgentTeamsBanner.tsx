import { ChevronDown, ChevronRight, FlaskConical } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'

const COLLAPSED_KEY = 'agent-teams-banner-collapsed'

export function AgentTeamsBanner() {
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem(COLLAPSED_KEY) === 'true')

  const toggle = () => {
    const next = !collapsed
    localStorage.setItem(COLLAPSED_KEY, String(next))
    setCollapsed(next)
  }

  return (
    <div
      className={cn(
        'rounded-xl border border-amber-200 dark:border-amber-800/60',
        'bg-amber-50/80 dark:bg-amber-950/30',
        'text-amber-800 dark:text-amber-300',
        'mb-4',
      )}
      role="note"
    >
      {/* Header row — always visible */}
      <button
        type="button"
        onClick={toggle}
        className={cn(
          'w-full flex items-center gap-2 px-4 py-2.5 text-left',
          'hover:bg-amber-100/60 dark:hover:bg-amber-900/20 transition-colors',
          collapsed ? 'rounded-xl' : 'rounded-t-xl',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 focus-visible:ring-inset',
        )}
        aria-expanded={!collapsed}
      >
        <FlaskConical
          className="w-4 h-4 flex-shrink-0 text-amber-600 dark:text-amber-400"
          aria-hidden="true"
        />
        <span className="flex-1 text-sm font-medium">Experimental Feature</span>
        {collapsed ? (
          <>
            <span className="text-xs opacity-60 mr-1">show setup</span>
            <ChevronRight className="w-3.5 h-3.5 opacity-60" aria-hidden="true" />
          </>
        ) : (
          <ChevronDown className="w-3.5 h-3.5 opacity-60" aria-hidden="true" />
        )}
      </button>

      {/* Expandable body */}
      {!collapsed && (
        <div className="px-4 pb-4 pt-0 space-y-3 text-sm">
          <p>
            Agent Teams is an experimental Claude Code feature. You need to enable it manually
            before it will work.
          </p>

          <div>
            <p className="text-xs font-medium opacity-70 mb-1.5">
              Add this to <code className="font-mono">~/.claude/settings.json</code>:
            </p>
            <pre
              className={cn(
                'text-xs font-mono rounded-lg px-3 py-2.5 leading-relaxed',
                'bg-amber-100/70 dark:bg-amber-900/30',
                'border border-amber-200 dark:border-amber-700/50',
              )}
            >
              {`{
  "env": {
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
  }
}`}
            </pre>
          </div>

          <p className="text-xs opacity-80">
            <span className="font-medium">Heads up:</span> Agent teams are temporary — they only
            exist while a session is running and may not always be saved here. Treat this page as
            best-effort history, not a complete record.
          </p>

          <a
            href="https://code.claude.com/docs/en/agent-teams"
            target="_blank"
            rel="noopener noreferrer"
            className={cn(
              'inline-flex items-center gap-1 text-xs font-medium',
              'text-amber-700 dark:text-amber-400',
              'hover:underline underline-offset-2',
            )}
          >
            Learn more about Agent Teams →
          </a>
        </div>
      )}
    </div>
  )
}
