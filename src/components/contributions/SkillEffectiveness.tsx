import { Zap, Check, AlertTriangle } from 'lucide-react'
import { cn } from '../../lib/utils'
import { InsightLine } from './InsightLine'
import type { SkillStats } from '../../types/generated'

interface SkillEffectivenessProps {
  bySkill: SkillStats[]
  skillInsight: string
}

/**
 * SkillEffectiveness displays a table comparing skill usage and outcomes.
 *
 * Shows sessions, average LOC, commit rate, and re-edit rate per skill.
 * Highlights best and worst performers.
 */
export function SkillEffectiveness({ bySkill, skillInsight }: SkillEffectivenessProps) {
  if (bySkill.length === 0) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center gap-2 mb-4">
          <Zap className="w-4 h-4 text-yellow-500" aria-hidden="true" />
          <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            Skill Impact
          </h2>
        </div>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          No skill usage data available for this period.
        </p>
      </div>
    )
  }

  // Find best and worst re-edit rates for comparison
  // Include skills that have measurable re-edit data (reeditRate > 0) or enough sessions to be statistically meaningful (> 2)
  const comparableSkills = bySkill.filter((s) => s.reeditRate > 0 || Number(s.sessions) > 2)
  const sortedByReedit = [...comparableSkills].sort((a, b) => a.reeditRate - b.reeditRate)
  // Best = lowest non-zero re-edit rate; worst = highest re-edit rate (need at least 2 skills to compare)
  const bestReedit = sortedByReedit.length > 0 ? sortedByReedit[0].reeditRate : null
  const worstReedit = sortedByReedit.length > 1 ? sortedByReedit[sortedByReedit.length - 1].reeditRate : bestReedit

  // Create insight object
  const insight = {
    text: skillInsight || 'Use skills to improve session outcomes',
    kind: 'tip' as const,
  }

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex items-center gap-2 mb-4">
        <Zap className="w-4 h-4 text-yellow-500" aria-hidden="true" />
        <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          Skill Impact
        </h2>
      </div>

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-gray-200 dark:border-gray-700">
              <th className="text-left py-2 pr-4 font-medium text-gray-500 dark:text-gray-400">
                Skill
              </th>
              <th className="text-right py-2 px-4 font-medium text-gray-500 dark:text-gray-400">
                Sessions
              </th>
              <th className="text-right py-2 px-4 font-medium text-gray-500 dark:text-gray-400">
                Avg LOC
              </th>
              <th className="text-right py-2 px-4 font-medium text-gray-500 dark:text-gray-400">
                Commit Rate
              </th>
              <th className="text-right py-2 pl-4 font-medium text-gray-500 dark:text-gray-400">
                Re-edit
              </th>
            </tr>
          </thead>
          <tbody>
            {bySkill.map((skill) => {
              const isBest = bestReedit !== null && skill.reeditRate === bestReedit && skill.reeditRate > 0
              // Only mark as worst if there are multiple comparable skills and this is the highest re-edit rate (not same as best)
              const isWorst = worstReedit !== null && skill.reeditRate === worstReedit && worstReedit !== bestReedit && comparableSkills.length > 1
              const isNoSkill = skill.skill === '(no skill)'

              return (
                <tr
                  key={skill.skill}
                  className={cn(
                    'border-b border-gray-100 dark:border-gray-800 last:border-0',
                    isNoSkill && 'opacity-75'
                  )}
                >
                  <td className="py-3 pr-4">
                    <span
                      className={cn(
                        'font-medium',
                        isNoSkill
                          ? 'text-gray-500 dark:text-gray-400 italic'
                          : 'text-gray-900 dark:text-gray-100'
                      )}
                    >
                      {skill.skill}
                    </span>
                  </td>
                  <td className="py-3 px-4 text-right tabular-nums text-gray-700 dark:text-gray-300">
                    {Number(skill.sessions).toLocaleString()}
                  </td>
                  <td className="py-3 px-4 text-right tabular-nums text-gray-700 dark:text-gray-300">
                    {Number(skill.avgLoc) > 0 ? `+${Number(skill.avgLoc).toLocaleString()}` : '--'}
                  </td>
                  <td className="py-3 px-4 text-right tabular-nums text-gray-700 dark:text-gray-300">
                    {formatPercent(skill.commitRate)}
                  </td>
                  <td className="py-3 pl-4 text-right">
                    <span className="inline-flex items-center gap-1">
                      <span
                        className={cn(
                          'tabular-nums',
                          isBest && 'text-green-600 dark:text-green-400 font-medium',
                          isWorst && 'text-amber-600 dark:text-amber-400',
                          !isBest && !isWorst && 'text-gray-700 dark:text-gray-300'
                        )}
                      >
                        {skill.reeditRate > 0 ? skill.reeditRate.toFixed(2) : '--'}
                      </span>
                      {isBest && (
                        <span className="inline-flex items-center gap-0.5 text-green-600 dark:text-green-400 text-xs">
                          <Check className="w-3 h-3" aria-hidden="true" />
                          best
                        </span>
                      )}
                      {isWorst && isNoSkill && (
                        <span className="inline-flex items-center gap-0.5 text-amber-600 dark:text-amber-400 text-xs">
                          <AlertTriangle className="w-3 h-3" aria-hidden="true" />
                          worst
                        </span>
                      )}
                    </span>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      {/* Insight */}
      {skillInsight && <InsightLine insight={insight} className="mt-4" />}
    </div>
  )
}

/**
 * Format a decimal rate as percentage.
 */
function formatPercent(rate: number): string {
  if (rate === 0) return '0%'
  return `${Math.round(rate * 100)}%`
}
