import { Activity, ArrowRight } from 'lucide-react'
import { Link } from 'react-router-dom'

interface LiveMonitorEmptyStateProps {
  /**
   * Claude Code processes the server has detected but that have not yet
   * reported a session via hooks. Drives the "detected, waiting" copy.
   */
  processCount: number
}

/**
 * First-run / no-active-sessions state for the Live Monitor.
 *
 * The live monitor is the product's hook, but a user with no *active* Claude
 * Code session would otherwise land on a blank board — and the default kanban
 * view shipped no empty guidance at all. This turns that dead-end into a clear
 * "what to do to make a session appear" plus a path into the history the user
 * already has indexed (the second most-used surface).
 *
 * Pure presentational. The server-derived `processCount` is injected so this
 * stays trivially testable and story-able.
 */
export function LiveMonitorEmptyState({ processCount }: LiveMonitorEmptyStateProps) {
  const hasProcesses = processCount > 0

  return (
    <div className="flex flex-col items-center justify-center text-center py-16 px-6">
      <div className="relative mb-5">
        <span
          className="absolute inset-0 -z-10 rounded-full bg-green-500/15 blur-xl animate-pulse"
          aria-hidden="true"
        />
        <Activity className="w-10 h-10 text-gray-400 dark:text-gray-500" strokeWidth={1.5} />
      </div>

      <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
        No Claude sessions running right now
      </h2>

      {hasProcesses ? (
        <>
          <div className="mt-3 inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-500/10 text-green-600 dark:text-green-400 text-sm font-medium">
            <span className="inline-block h-2 w-2 rounded-full bg-green-500 animate-pulse" />
            {processCount} Claude {processCount === 1 ? 'process' : 'processes'} detected
          </div>
          <p className="mt-3 max-w-sm text-sm text-gray-500 dark:text-gray-400">
            Sessions appear as they report in via hooks. Try sending a message in one of your Claude
            Code terminals.
          </p>
        </>
      ) : (
        <>
          <p className="mt-2 max-w-sm text-sm text-gray-500 dark:text-gray-400">
            Start a session in your terminal and it streams in here the moment Claude Code reports
            its first message.
          </p>
          <div className="mt-4 inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-gray-500/10 text-gray-500 dark:text-gray-400 text-xs font-medium">
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-green-500 animate-pulse" />
            Watching for sessions…
          </div>
        </>
      )}

      <div className="mt-8 pt-6 border-t border-gray-200 dark:border-gray-800 w-full max-w-sm">
        <p className="mb-3 text-xs text-gray-400 dark:text-gray-500">
          Or pick up where you left off
        </p>
        <Link
          to="/sessions"
          className="inline-flex items-center gap-2 px-4 py-2.5 rounded-lg bg-indigo-500 text-sm font-medium text-white transition-colors hover:bg-indigo-400"
        >
          Browse your past sessions
          <ArrowRight className="w-4 h-4" />
        </Link>
        <p className="mt-3 text-xs text-gray-400 dark:text-gray-500">
          Every past session is indexed and searchable.
        </p>
      </div>
    </div>
  )
}
