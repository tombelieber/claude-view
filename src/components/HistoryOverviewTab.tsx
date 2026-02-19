import type { SessionDetail } from '../types/generated'
import type { RichSessionData } from '../types/generated/RichSessionData'
import type { SessionInfo } from '../types/generated'
import { ContextGauge } from './live/ContextGauge'
import { SessionMetricsBar } from './SessionMetricsBar'
import { FilesTouchedPanel, buildFilesTouched } from './FilesTouchedPanel'
import { CommitsPanel } from './CommitsPanel'

interface HistoryOverviewTabProps {
  sessionDetail: SessionDetail
  /** SessionInfo from sessions list (used for metrics bar) */
  sessionInfo: SessionInfo | undefined
  richData: RichSessionData | undefined
  isLoadingRich: boolean
}

export function HistoryOverviewTab({ sessionDetail, sessionInfo, richData, isLoadingRich }: HistoryOverviewTabProps) {
  return (
    <div className="space-y-4">
      {/* Rich data section */}
      {isLoadingRich ? (
        <div className="text-xs text-gray-400 animate-pulse">Loading session data...</div>
      ) : richData ? (
        <div className="space-y-3">
          {/* Context gauge — use compact bar mode for sidebar */}
          <div>
            <div className="text-xs text-gray-500 dark:text-gray-400 mb-1">Context Window</div>
            <ContextGauge
              contextWindowTokens={richData.contextWindowTokens}
              model={richData.model}
              group="needs_you"
            />
          </div>
          {/* Cost summary */}
          <div className="flex items-center justify-between text-xs">
            <span className="text-gray-500 dark:text-gray-400">Total Cost</span>
            <span className="font-mono text-gray-700 dark:text-gray-300">
              ${richData.cost.totalUsd.toFixed(4)}
            </span>
          </div>
          {/* Model */}
          {richData.model && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-gray-500 dark:text-gray-400">Model</span>
              <span className="text-gray-700 dark:text-gray-300">{richData.model}</span>
            </div>
          )}
          {/* Cache status */}
          <div className="flex items-center justify-between text-xs">
            <span className="text-gray-500 dark:text-gray-400">Cache</span>
            <span className={
              richData.cacheStatus === 'warm' ? 'text-green-500' :
              richData.cacheStatus === 'cold' ? 'text-red-400' :
              'text-gray-400'
            }>
              {richData.cacheStatus === 'warm' ? 'Warm' :
               richData.cacheStatus === 'cold' ? 'Cold' : '\u2014'}
            </span>
          </div>
        </div>
      ) : null}

      {/* Session metrics (vertical layout) */}
      {sessionInfo && sessionInfo.userPromptCount > 0 && (
        <SessionMetricsBar
          prompts={sessionInfo.userPromptCount}
          tokens={
            sessionInfo.totalInputTokens != null && sessionInfo.totalOutputTokens != null
              ? BigInt(sessionInfo.totalInputTokens) + BigInt(sessionInfo.totalOutputTokens)
              : null
          }
          filesRead={sessionInfo.filesReadCount}
          filesEdited={sessionInfo.filesEditedCount}
          reeditRate={
            sessionInfo.filesEditedCount > 0
              ? sessionInfo.reeditedFilesCount / sessionInfo.filesEditedCount
              : null
          }
          commits={sessionInfo.commitCount}
          variant="vertical"
        />
      )}

      {/* Files Touched */}
      <FilesTouchedPanel
        files={buildFilesTouched(
          sessionDetail.filesRead ?? [],
          sessionDetail.filesEdited ?? []
        )}
      />

      {/* Linked Commits */}
      <CommitsPanel commits={sessionDetail.commits ?? []} />
    </div>
  )
}
