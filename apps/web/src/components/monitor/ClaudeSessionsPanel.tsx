import type { SessionResource } from '../../types/generated/SessionResource'
import type { LiveSession } from '../live/use-live-sessions'
import { sessionTotalCost } from '../live/use-live-sessions'
import { SessionRow } from './SessionRow'

interface ClaudeSessionsPanelProps {
  sessionResources: SessionResource[]
  liveSessions: LiveSession[]
}

export function ClaudeSessionsPanel({ sessionResources, liveSessions }: ClaudeSessionsPanelProps) {
  // Build lookup for live session metadata by session ID
  const sessionMap = new Map(liveSessions.map((s) => [s.id, s]))

  if (sessionResources.length === 0) {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-6">
        <div className="flex items-center gap-2 mb-4">
          <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            Claude Sessions
          </h2>
          <span className="text-xs bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 px-1.5 py-0.5 rounded-full">
            0
          </span>
        </div>
        <p className="text-sm text-gray-400 dark:text-gray-500 text-center py-6">
          No active Claude sessions
        </p>
      </div>
    )
  }

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4">
      <div className="flex items-center gap-2 mb-3">
        <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Claude Sessions</h2>
        <span className="text-xs bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 px-1.5 py-0.5 rounded-full font-medium">
          {sessionResources.length}
        </span>
      </div>
      <div className="flex flex-col gap-2">
        {sessionResources.map((res) => {
          const live = sessionMap.get(res.sessionId)
          return (
            <SessionRow
              key={res.sessionId}
              resource={res}
              status={live?.status}
              projectName={live?.projectDisplayName || live?.project}
              branch={live?.effectiveBranch ?? live?.gitBranch ?? undefined}
              costUsd={live ? sessionTotalCost(live) : undefined}
              totalTokens={live?.tokens.totalTokens}
              turnCount={live?.turnCount}
            />
          )
        })}
      </div>
    </div>
  )
}
