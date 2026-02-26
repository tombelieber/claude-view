import { Bar, BarChart, Cell, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'
import type { ProjectActivity } from '../../lib/activity-utils'
import { formatHumanDuration } from '../../lib/format-utils'

const BAR_COLORS = [
  '#3b82f6', // blue-500
  '#8b5cf6', // violet-500
  '#06b6d4', // cyan-500
  '#f59e0b', // amber-500
  '#10b981', // emerald-500
  '#ef4444', // red-500
  '#ec4899', // pink-500
  '#6b7280', // gray-500
]

interface ProjectBreakdownProps {
  projects: ProjectActivity[]
  onProjectClick?: (projectPath: string | null) => void
  selectedProject?: string | null
}

export function ProjectBreakdown({
  projects,
  onProjectClick,
  selectedProject,
}: ProjectBreakdownProps) {
  if (projects.length === 0) {
    return null
  }

  const totalSeconds = projects.reduce((sum, p) => sum + p.totalSeconds, 0)

  // Show top 8 projects max
  const displayProjects = projects.slice(0, 8)
  const chartData = displayProjects.map((p) => ({
    name: p.name,
    seconds: p.totalSeconds,
    projectPath: p.projectPath,
    label: `${formatHumanDuration(p.totalSeconds)} (${totalSeconds > 0 ? Math.round((p.totalSeconds / totalSeconds) * 100) : 0}%)`,
  }))

  const chartHeight = Math.max(displayProjects.length * 36, 100)

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-200">By Project</h2>
        {selectedProject && (
          <button
            type="button"
            onClick={() => onProjectClick?.(null)}
            className="text-xs text-blue-500 hover:text-blue-600 cursor-pointer"
          >
            Clear filter
          </button>
        )}
      </div>
      <div style={{ height: chartHeight }}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart
            data={chartData}
            layout="vertical"
            margin={{ left: 10, right: 8, top: 0, bottom: 0 }}
          >
            <XAxis type="number" hide />
            <YAxis
              type="category"
              dataKey="name"
              width={120}
              tick={{ fontSize: 12, fill: 'currentColor' }}
              tickLine={false}
              axisLine={false}
              className="text-gray-600 dark:text-gray-300"
            />
            <Tooltip
              content={({ payload }) => {
                if (!payload?.[0]) return null
                const d = payload[0].payload
                return (
                  <div className="bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 text-xs px-3 py-2 rounded-lg shadow-lg">
                    <p className="font-medium">{d.name}</p>
                    <p>{d.label}</p>
                  </div>
                )
              }}
            />
            <Bar
              dataKey="seconds"
              radius={[0, 4, 4, 0]}
              cursor="pointer"
              onClick={(_, index) => {
                const entry = chartData[index]
                if (entry?.projectPath) {
                  onProjectClick?.(selectedProject === entry.projectPath ? null : entry.projectPath)
                }
              }}
            >
              {chartData.map((entry, i) => (
                <Cell
                  key={entry.projectPath}
                  fill={BAR_COLORS[i % BAR_COLORS.length]}
                  opacity={selectedProject && selectedProject !== entry.projectPath ? 0.3 : 1}
                />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}
