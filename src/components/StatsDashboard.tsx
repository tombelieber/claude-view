import { useMemo } from 'react'
import { useOutletContext, useNavigate, Link } from 'react-router-dom'
import { BarChart3, Zap, FolderOpen, Calendar, Pencil, Eye, Terminal } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface OutletContext {
  projects: ProjectInfo[]
}

export function StatsDashboard() {
  const { projects } = useOutletContext<OutletContext>()
  const navigate = useNavigate()

  const stats = useMemo(() => {
    const allSessions = projects.flatMap(p => p.sessions)

    // Aggregate skills
    const skillCounts = new Map<string, number>()
    let totalEdits = 0, totalReads = 0, totalBash = 0

    for (const session of allSessions) {
      for (const skill of session.skillsUsed ?? []) {
        skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
      }
      const tc = session.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
      totalEdits += tc.edit + tc.write
      totalReads += tc.read
      totalBash += tc.bash
    }

    const topSkills = Array.from(skillCounts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
    const maxSkillCount = topSkills[0]?.[1] || 1

    // Project stats
    const projectStats = projects
      .map(p => ({
        name: p.displayName,
        fullName: p.name,
        sessions: p.sessions.length,
      }))
      .sort((a, b) => b.sessions - a.sessions)
      .slice(0, 5)
    const maxProjectSessions = projectStats[0]?.sessions || 1

    // Date range
    const dates = allSessions.map(s => new Date(s.modifiedAt))
    const earliest = dates.reduce((min, d) => d < min ? d : min, new Date())

    // Activity heatmap (last 30 days)
    const heatmap = generateHeatmap(allSessions)

    return {
      totalSessions: allSessions.length,
      totalProjects: projects.length,
      since: earliest.toLocaleDateString('en-US', { month: 'short', year: 'numeric' }),
      topSkills,
      maxSkillCount,
      projectStats,
      maxProjectSessions,
      tools: { edits: totalEdits, reads: totalReads, bash: totalBash },
      heatmap,
    }
  }, [projects])

  const handleSkillClick = (skill: string) => {
    navigate(`/search?q=${encodeURIComponent(`skill:${skill.replace('/', '')}`)}`)
  }

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      {/* Header Card */}
      <div className="bg-white rounded-xl border border-gray-200 p-6">
        <div className="flex items-center gap-2 mb-4">
          <BarChart3 className="w-5 h-5 text-[#7c9885]" />
          <h1 className="text-xl font-semibold text-gray-900">Your Claude Code Usage</h1>
        </div>

        <div className="flex items-center gap-6 text-sm text-gray-600">
          <div>
            <span className="text-2xl font-bold text-gray-900 tabular-nums">{stats.totalSessions}</span>
            <span className="ml-1">sessions</span>
          </div>
          <div className="w-px h-8 bg-gray-200" />
          <div>
            <span className="text-2xl font-bold text-gray-900 tabular-nums">{stats.totalProjects}</span>
            <span className="ml-1">projects</span>
          </div>
          <div className="w-px h-8 bg-gray-200" />
          <div className="text-gray-500">
            since {stats.since}
          </div>
        </div>
      </div>

      <div className="grid md:grid-cols-2 gap-6">
        {/* Top Skills */}
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5">
            <Zap className="w-4 h-4" />
            Top Skills
          </h2>
          <div className="space-y-3">
            {stats.topSkills.map(([skill, count]) => (
              <button
                key={skill}
                onClick={() => handleSkillClick(skill)}
                className="w-full group text-left"
              >
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="font-mono text-gray-700 group-hover:text-blue-600 transition-colors">
                    {skill}
                  </span>
                  <span className="tabular-nums text-gray-400">{count}</span>
                </div>
                <div className="h-2 bg-gray-100 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-[#7c9885] group-hover:bg-blue-500 transition-colors rounded-full"
                    style={{ width: `${(count / stats.maxSkillCount) * 100}%` }}
                  />
                </div>
              </button>
            ))}
            {stats.topSkills.length === 0 && (
              <p className="text-sm text-gray-400 italic">No skills used yet</p>
            )}
          </div>
        </div>

        {/* Most Active Projects */}
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5">
            <FolderOpen className="w-4 h-4" />
            Most Active Projects
          </h2>
          <div className="space-y-3">
            {stats.projectStats.map((project) => (
              <Link
                key={project.fullName}
                to={`/project/${encodeURIComponent(project.fullName)}`}
                className="w-full group block"
              >
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="text-gray-700 group-hover:text-blue-600 transition-colors">
                    {project.name}
                  </span>
                  <span className="tabular-nums text-gray-400">{project.sessions}</span>
                </div>
                <div className="h-2 bg-gray-100 rounded-full overflow-hidden">
                  <div
                    className="h-full rounded-full transition-colors bg-gray-300 group-hover:bg-blue-500"
                    style={{ width: `${(project.sessions / stats.maxProjectSessions) * 100}%` }}
                  />
                </div>
              </Link>
            ))}
          </div>
        </div>
      </div>

      {/* Activity Heatmap */}
      <div className="bg-white rounded-xl border border-gray-200 p-6">
        <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5">
          <Calendar className="w-4 h-4" />
          Activity (Last 30 Days)
        </h2>
        <ActivityHeatmap data={stats.heatmap} navigate={navigate} />
      </div>

      {/* Global Tool Usage */}
      <div className="bg-white rounded-xl border border-gray-200 p-6">
        <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4">
          Tool Usage
        </h2>
        <div className="grid grid-cols-3 gap-4">
          {[
            { label: 'Edits', value: stats.tools.edits, icon: Pencil, color: 'text-blue-500' },
            { label: 'Reads', value: stats.tools.reads, icon: Eye, color: 'text-green-500' },
            { label: 'Bash', value: stats.tools.bash, icon: Terminal, color: 'text-amber-500' },
          ].map(({ label, value, icon: Icon, color }) => (
            <div key={label} className="text-center p-4 bg-gray-50 rounded-lg">
              <Icon className={cn('w-6 h-6 mx-auto mb-2', color)} />
              <p className="text-2xl font-bold text-gray-900 tabular-nums">{value}</p>
              <p className="text-xs text-gray-500">{label}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

// Helper: Generate heatmap data for last 30 days
function generateHeatmap(sessions: { modifiedAt: string }[]) {
  const days: { date: Date; count: number }[] = []
  const now = new Date()

  for (let i = 29; i >= 0; i--) {
    const date = new Date(now)
    date.setDate(date.getDate() - i)
    date.setHours(0, 0, 0, 0)
    days.push({ date, count: 0 })
  }

  for (const session of sessions) {
    const sessionDate = new Date(session.modifiedAt)
    sessionDate.setHours(0, 0, 0, 0)
    const dayEntry = days.find(d => d.date.getTime() === sessionDate.getTime())
    if (dayEntry) dayEntry.count++
  }

  return days
}

// Activity Heatmap Component
function ActivityHeatmap({
  data,
  navigate
}: {
  data: { date: Date; count: number }[]
  navigate: (path: string) => void
}) {
  const maxCount = Math.max(...data.map(d => d.count), 1)

  const getColor = (count: number) => {
    if (count === 0) return 'bg-gray-100'
    const intensity = count / maxCount
    if (intensity > 0.66) return 'bg-green-500'
    if (intensity > 0.33) return 'bg-green-300'
    return 'bg-green-200'
  }

  const handleDayClick = (date: Date) => {
    const dateStr = date.toISOString().split('T')[0]
    const nextDay = new Date(date)
    nextDay.setDate(nextDay.getDate() + 1)
    const nextDateStr = nextDay.toISOString().split('T')[0]
    navigate(`/search?q=${encodeURIComponent(`after:${dateStr} before:${nextDateStr}`)}`)
  }

  // Group by week
  const weeks: { date: Date; count: number }[][] = []
  let currentWeek: { date: Date; count: number }[] = []

  for (const day of data) {
    if (currentWeek.length === 7) {
      weeks.push(currentWeek)
      currentWeek = []
    }
    currentWeek.push(day)
  }
  if (currentWeek.length > 0) weeks.push(currentWeek)

  return (
    <div className="flex gap-1">
      {weeks.map((week, wi) => (
        <div key={wi} className="flex flex-col gap-1">
          {week.map((day) => (
            <button
              key={day.date.toISOString()}
              onClick={() => handleDayClick(day.date)}
              className={cn(
                'w-3 h-3 rounded-sm transition-colors hover:ring-2 hover:ring-blue-400',
                getColor(day.count)
              )}
              title={`${day.date.toLocaleDateString()}: ${day.count} sessions`}
            />
          ))}
        </div>
      ))}
      <div className="ml-2 flex items-center gap-2 text-xs text-gray-400">
        <span>Less</span>
        <div className="flex gap-0.5">
          <div className="w-3 h-3 rounded-sm bg-gray-100" />
          <div className="w-3 h-3 rounded-sm bg-green-200" />
          <div className="w-3 h-3 rounded-sm bg-green-300" />
          <div className="w-3 h-3 rounded-sm bg-green-500" />
        </div>
        <span>More</span>
      </div>
    </div>
  )
}
