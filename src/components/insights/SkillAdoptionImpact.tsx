import { useState } from 'react'
import { cn } from '../../lib/utils'
import { Zap } from 'lucide-react'
import type { SkillAdoption } from '../../types/generated/SkillAdoption'

interface SkillAdoptionImpactProps {
  skills: SkillAdoption[]
  className?: string
}

function formatAdoptionDate(isoDate: string): string {
  try {
    const date = new Date(isoDate)
    if (isNaN(date.getTime())) return '--'
    const months = [
      'Jan',
      'Feb',
      'Mar',
      'Apr',
      'May',
      'Jun',
      'Jul',
      'Aug',
      'Sep',
      'Oct',
      'Nov',
      'Dec',
    ]
    return `${months[date.getMonth()]} ${date.getDate()}`
  } catch {
    return '--'
  }
}

function generateSkillInsight(skill: SkillAdoption): string {
  const { learningCurve, impactOnReedit, skill: skillName } = skill

  // Find inflection point (where rate stabilizes)
  const inflectionPoint = learningCurve.findIndex((d, i, arr) => {
    if (i < 2 || i >= arr.length - 1) return false
    const prevDelta = arr[i - 1].reeditRate - arr[i - 2].reeditRate
    const currDelta = d.reeditRate - arr[i - 1].reeditRate
    return Math.abs(currDelta) < Math.abs(prevDelta) * 0.5
  })

  if (inflectionPoint > 0) {
    return `${skillName} took ~${inflectionPoint} sessions to show benefits -- stick with new skills, improvement comes with practice`
  }

  if (impactOnReedit < -30) {
    return `${skillName} has dramatically improved your workflow -- consider using it more consistently`
  }

  if (impactOnReedit < 0) {
    return `${skillName} is contributing to your improvement -- keep using it to build mastery`
  }

  return `${skillName} hasn't shown a measurable impact yet -- it may take more sessions or different use patterns`
}

export function SkillAdoptionImpact({ skills, className }: SkillAdoptionImpactProps) {
  const [selectedSkill, setSelectedSkill] = useState<string | null>(
    skills.length > 0 ? skills[0].skill : null
  )

  // Sort by impact (most beneficial first = most negative)
  const sorted = [...skills].sort((a, b) => a.impactOnReedit - b.impactOnReedit)

  const selectedData = skills.find((s) => s.skill === selectedSkill)

  const maxImpact = Math.max(...skills.map((s) => Math.abs(s.impactOnReedit)), 1)
  const barScale = 100 / maxImpact

  if (skills.length === 0) {
    return (
      <div
        className={cn(
          'bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
          className
        )}
      >
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
          Skill Adoption Impact
        </h3>
        <div className="py-6 text-center">
          <Zap className="w-8 h-8 text-gray-300 dark:text-gray-600 mx-auto mb-3" />
          <p className="text-sm text-gray-500 dark:text-gray-400">
            No skill adoption data yet. Use skills in 3+ sessions to see their impact.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div
      className={cn(
        'bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
        className
      )}
    >
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        Skill Adoption Impact
      </h3>

      {/* Skills table */}
      <div className="overflow-x-auto">
        <table className="w-full mb-6" role="table">
          <thead>
            <tr className="text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
              <th className="pb-3">Skill</th>
              <th className="pb-3">Adopted</th>
              <th className="pb-3 text-right">Sessions</th>
              <th className="pb-3 px-4">Impact on Re-edit</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
            {sorted.map((skill) => (
              <tr
                key={skill.skill}
                className={cn(
                  'cursor-pointer transition-colors',
                  selectedSkill === skill.skill
                    ? 'bg-blue-50 dark:bg-blue-900/20'
                    : 'hover:bg-gray-50 dark:hover:bg-gray-800'
                )}
                onClick={() => setSelectedSkill(skill.skill)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    setSelectedSkill(skill.skill)
                  }
                }}
                aria-selected={selectedSkill === skill.skill}
                aria-label={`${skill.skill} skill, impact ${skill.impactOnReedit}%`}
              >
                <td className="py-2.5 text-sm font-medium text-gray-700 dark:text-gray-300">
                  {skill.skill}
                </td>
                <td className="py-2.5 text-sm text-gray-500 dark:text-gray-400">
                  {formatAdoptionDate(skill.adoptedAt)}
                </td>
                <td className="py-2.5 text-sm font-mono text-right text-gray-600 dark:text-gray-400">
                  {skill.sessionCount}
                </td>
                <td className="py-2.5 px-4">
                  <div className="flex items-center gap-2">
                    <span
                      className={cn(
                        'text-sm font-mono min-w-[3.5rem] text-right',
                        skill.impactOnReedit < 0
                          ? 'text-green-600 dark:text-green-400'
                          : 'text-amber-600 dark:text-amber-400'
                      )}
                    >
                      {skill.impactOnReedit > 0 ? '+' : ''}
                      {skill.impactOnReedit.toFixed(0)}%
                    </span>
                    <div className="flex-1 h-2 bg-gray-100 dark:bg-gray-800 rounded overflow-hidden">
                      <div
                        className={cn(
                          'h-full rounded',
                          skill.impactOnReedit < 0 ? 'bg-green-500' : 'bg-amber-500'
                        )}
                        style={{
                          width: `${Math.min(Math.abs(skill.impactOnReedit) * barScale, 100)}%`,
                        }}
                      />
                    </div>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Learning curve chart */}
      {selectedData && selectedData.learningCurve.length > 1 && (
        <div className="border-t border-gray-200 dark:border-gray-700 pt-4">
          <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
            Learning Curve ({selectedData.skill})
          </h4>
          <LearningCurveChart data={selectedData.learningCurve} />
        </div>
      )}

      {/* Insight */}
      {selectedData && (
        <div className="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
          <p className="text-sm text-blue-800 dark:text-blue-200">
            {generateSkillInsight(selectedData)}
          </p>
        </div>
      )}
    </div>
  )
}

// ============================================================================
// Learning Curve Chart (SVG)
// ============================================================================

function LearningCurveChart({
  data,
}: {
  data: Array<{ session: number; reeditRate: number }>
}) {
  const chartHeight = 120
  const paddingY = 16
  const paddingX = 40

  const maxRate = Math.max(...data.map((d) => d.reeditRate))
  const minRate = Math.min(...data.map((d) => d.reeditRate))
  const range = maxRate - minRate || 0.1

  const points = data.map((d, i) => ({
    x: paddingX + (i / Math.max(data.length - 1, 1)) * (100 - paddingX * 2),
    y: paddingY + ((maxRate - d.reeditRate) / range) * (chartHeight - paddingY * 2),
    label: d.reeditRate,
    session: d.session,
  }))

  // Use percentage-based x positions in viewBox
  const viewBoxWidth = 100
  const viewBoxHeight = chartHeight

  const pathD = points
    .map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`)
    .join(' ')

  return (
    <div className="relative" style={{ height: chartHeight }}>
      <svg
        viewBox={`0 0 ${viewBoxWidth} ${viewBoxHeight}`}
        preserveAspectRatio="none"
        className="w-full overflow-visible"
        style={{ height: chartHeight }}
        role="img"
        aria-label="Learning curve chart showing re-edit rate over sessions"
      >
        {/* Grid lines */}
        <line
          x1={paddingX}
          y1={paddingY}
          x2={viewBoxWidth - paddingX}
          y2={paddingY}
          stroke="currentColor"
          className="text-gray-200 dark:text-gray-700"
          strokeWidth="0.3"
        />
        <line
          x1={paddingX}
          y1={viewBoxHeight / 2}
          x2={viewBoxWidth - paddingX}
          y2={viewBoxHeight / 2}
          stroke="currentColor"
          className="text-gray-200 dark:text-gray-700"
          strokeWidth="0.3"
          strokeDasharray="2"
        />
        <line
          x1={paddingX}
          y1={viewBoxHeight - paddingY}
          x2={viewBoxWidth - paddingX}
          y2={viewBoxHeight - paddingY}
          stroke="currentColor"
          className="text-gray-200 dark:text-gray-700"
          strokeWidth="0.3"
        />

        {/* Line */}
        <path
          d={pathD}
          fill="none"
          stroke="currentColor"
          className="text-blue-500"
          strokeWidth="0.8"
          vectorEffect="non-scaling-stroke"
        />

        {/* Points */}
        {points.map((p, i) => (
          <circle
            key={i}
            cx={p.x}
            cy={p.y}
            r="1.2"
            fill="currentColor"
            className="text-blue-500"
          >
            <title>
              Session {p.session}: {p.label.toFixed(2)}
            </title>
          </circle>
        ))}
      </svg>

      {/* Y-axis labels */}
      <div className="absolute left-0 top-1 text-[10px] text-gray-500 dark:text-gray-400 font-mono">
        {maxRate.toFixed(2)}
      </div>
      <div className="absolute left-0 bottom-1 text-[10px] text-gray-500 dark:text-gray-400 font-mono">
        {minRate.toFixed(2)}
      </div>

      {/* X-axis label */}
      <div className="absolute right-0 bottom-0 text-[10px] text-gray-500 dark:text-gray-400">
        sessions
      </div>
    </div>
  )
}
