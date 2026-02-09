import { useMemo } from 'react'
import { Zap, Check, AlertTriangle, ChevronRight } from 'lucide-react'
import { BarChart, Bar, XAxis, YAxis, Tooltip as RechartsTooltip, ResponsiveContainer, Cell, LabelList } from 'recharts'
import { cn } from '../../lib/utils'
import { InsightLine } from './InsightLine'
import type { SkillStats } from '../../types/generated'
import { MetricTooltip } from './MetricTooltip'

interface SkillEffectivenessProps {
  bySkill: SkillStats[]
  skillInsight: string
}

/**
 * Color-code bar by commit rate quality.
 * Gray for "(no skill)" baseline, green for high commit rate (>50%),
 * amber for moderate (>0%), gray otherwise.
 */
function getSkillBarColor(skill: { skill: string; commitRate: number }): string {
  if (skill.skill === '(no skill)') return '#9ca3af' // gray-400 baseline
  if (skill.commitRate > 0.5) return '#22c55e' // green-500 high quality
  if (skill.commitRate > 0) return '#f59e0b' // amber-500 moderate
  return '#9ca3af' // gray-400
}

/**
 * Format a decimal rate as percentage.
 */
function formatPercent(rate: number): string {
  if (rate === 0) return '0%'
  return `${Math.round(rate * 100)}%`
}

/**
 * SkillEffectiveness displays a horizontal bar chart of sessions per skill
 * and a collapsible detailed table comparing skill usage and outcomes.
 *
 * Shows sessions, average LOC, commit rate, and re-edit rate per skill.
 * Highlights best and worst performers.
 */
export function SkillEffectiveness({ bySkill, skillInsight }: SkillEffectivenessProps) {
  // Sort skills by sessions descending for chart
  const chartData = useMemo(() =>
    [...bySkill]
      .sort((a, b) => b.sessions - a.sessions)
      .map(s => ({
        skill: s.skill,
        sessions: s.sessions,
        commitRate: s.commitRate,
        avgLoc: s.avgLoc,
        reeditRate: s.reeditRate,
      })),
    [bySkill]
  )

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
  const comparableSkills = bySkill.filter((s) => s.reeditRate > 0 || s.sessions > 2)
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

      {/* Horizontal Bar Chart */}
      <div style={{ height: `${chartData.length * 40 + 32}px` }} className="mb-4 overflow-hidden">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData} layout="vertical" margin={{ top: 5, right: 80, left: 0, bottom: 5 }}>
            <XAxis type="number" tick={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }} />
            <YAxis type="category" dataKey="skill" tick={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }} width={180} />
            <RechartsTooltip
              contentStyle={{
                backgroundColor: 'var(--tooltip-bg, #fff)',
                border: '1px solid var(--tooltip-border, #e5e7eb)',
                borderRadius: '8px',
                fontSize: '12px',
              }}
              formatter={(value: number, _name: string, props: any) => {
                const item = props.payload
                const parts = [`${value} sessions`]
                if (item.avgLoc > 0) parts.push(`Avg LOC: +${item.avgLoc}`)
                parts.push(`Commit: ${formatPercent(item.commitRate)}`)
                if (item.reeditRate > 0) parts.push(`Re-edit: ${item.reeditRate.toFixed(2)}`)
                return [parts.join(' \u00b7 '), item.skill]
              }}
            />
            <Bar dataKey="sessions" radius={[0, 4, 4, 0]}>
              {chartData.map((entry) => (
                <Cell key={entry.skill} fill={getSkillBarColor(entry)} />
              ))}
              <LabelList
                dataKey="sessions"
                position="right"
                formatter={(v: number) => `${v} sessions`}
                style={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }}
              />
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>

      {/* Collapsible Detailed Table */}
      <details className="group">
        <summary className="text-sm text-gray-500 dark:text-gray-400 cursor-pointer hover:text-gray-700 dark:hover:text-gray-300 motion-safe:transition-colors motion-safe:duration-200 list-none flex items-center gap-1">
          <ChevronRight className="w-4 h-4 motion-safe:transition-transform motion-safe:duration-200 group-open:rotate-90" />
          Show detailed table
        </summary>
        <div className="mt-3 overflow-x-auto">
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
                  <span className="inline-flex items-center">
                    Re-edit
                    <MetricTooltip>
                      <span className="font-medium text-gray-900 dark:text-gray-100">Re-edit rate</span> measures how often AI-generated files need further editing after the initial write.
                      <br /><br />
                      <span className="font-medium text-gray-900 dark:text-gray-100">Lower is better.</span> 0 = no re-edits needed.
                      <br /><br />
                      Formula: files re-edited / total files edited
                    </MetricTooltip>
                  </span>
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
                      {skill.sessions.toLocaleString()}
                    </td>
                    <td className="py-3 px-4 text-right tabular-nums text-gray-700 dark:text-gray-300">
                      {skill.avgLoc > 0 ? `+${skill.avgLoc.toLocaleString()}` : '--'}
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
      </details>

      {/* Insight */}
      {skillInsight && <InsightLine insight={insight} className="mt-4" />}
    </div>
  )
}
