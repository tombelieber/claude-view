import { useQuery } from '@tanstack/react-query'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import type { PromptStats } from '../../types/generated/PromptStats'

const DAY_NAMES = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']

function formatHour(h: number): string {
  if (h === 0) return '12 AM'
  if (h < 12) return `${h} AM`
  if (h === 12) return '12 PM'
  return `${h - 12} PM`
}

export function PromptProfile() {
  const [collapsed, setCollapsed] = useState(false)

  const { data: stats, isLoading } = useQuery({
    queryKey: ['prompt-stats'],
    queryFn: () => fetch('/api/prompts/stats').then((r) => r.json()) as Promise<PromptStats>,
  })

  // Sort intents by count descending for the bar chart
  const sortedIntents = stats
    ? Object.entries(stats.intentBreakdown)
        .filter((e): e is [string, number] => e[1] != null)
        .sort((a, b) => b[1] - a[1])
    : []
  const maxIntentCount = sortedIntents.length > 0 ? sortedIntents[0][1] : 1

  return (
    <div>
      {/* Section header */}
      <button
        type="button"
        onClick={() => setCollapsed((c) => !c)}
        className="flex items-center gap-1.5 w-full text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 transition-colors cursor-pointer"
      >
        {collapsed ? (
          <ChevronRight className="w-3.5 h-3.5 shrink-0" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 shrink-0" />
        )}
        <span>Prompt Profile</span>
      </button>

      {collapsed ? null : (
        <div className="mt-3 space-y-4">
          {isLoading || !stats ? (
            <p className="text-xs text-gray-400 dark:text-gray-500">Loading stats...</p>
          ) : (
            <>
              {/* Stat cards grid */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                <StatBox label="Total Prompts" value={stats.totalPrompts.toLocaleString()} />
                <StatBox label="Unique Projects" value={String(stats.uniqueProjects)} />
                <StatBox label="Days of History" value={String(Number(stats.daysCovered))} />
                <StatBox label="Night Owl %" value={`${Math.round(stats.nightOwlRatio * 100)}%`} />
              </div>

              {/* Intent breakdown */}
              {sortedIntents.length > 0 && (
                <div>
                  <p className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-2">
                    Intent Breakdown
                  </p>
                  <div className="space-y-1.5">
                    {sortedIntents.map(([intent, count]) => (
                      <div key={intent} className="flex items-center gap-2 text-xs">
                        <span className="w-16 text-gray-600 dark:text-gray-400 truncate text-right">
                          {intent}
                        </span>
                        <div className="flex-1 h-4 bg-gray-100 dark:bg-gray-800 rounded overflow-hidden">
                          <div
                            className="h-full bg-blue-500 dark:bg-blue-400 rounded"
                            style={{ width: `${(count / maxIntentCount) * 100}%` }}
                          />
                        </div>
                        <span className="w-8 text-gray-500 dark:text-gray-400 tabular-nums">
                          {count}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Peak time summary */}
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Peak activity:{' '}
                <span className="text-gray-700 dark:text-gray-300 font-medium">
                  {formatHour(stats.peakHour)}
                </span>
                {' on '}
                <span className="text-gray-700 dark:text-gray-300 font-medium">
                  {DAY_NAMES[stats.peakDay] ?? `Day ${stats.peakDay}`}
                </span>
                {' \u00B7 '}
                <span className="text-gray-700 dark:text-gray-300 font-medium">
                  {stats.promptsPerDay.toFixed(1)}
                </span>
                {' prompts/day'}
              </p>
            </>
          )}
        </div>
      )}
    </div>
  )
}

function StatBox({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-gray-50 dark:bg-white/[0.05] rounded-lg p-3 text-center ring-1 ring-gray-950/[0.05] dark:ring-white/[0.06]">
      <p className="text-xs text-gray-500 dark:text-gray-400 mb-0.5">{label}</p>
      <p className="text-lg font-semibold text-gray-800 dark:text-gray-100 tabular-nums">{value}</p>
    </div>
  )
}
