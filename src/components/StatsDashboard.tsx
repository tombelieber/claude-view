import { useMemo } from 'react'
import { BarChart3, Zap, FolderOpen } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface StatsDashboardProps {
  projects: ProjectInfo[]
  onFilterClick: (query: string) => void
}

export function StatsDashboard({ projects, onFilterClick }: StatsDashboardProps) {
  const stats = useMemo(() => {
    const allSessions = projects.flatMap(p => p.sessions)

    // Aggregate skills across all sessions
    const skillCounts = new Map<string, number>()
    for (const session of allSessions) {
      for (const skill of session.skillsUsed) {
        skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
      }
    }
    const topSkills = Array.from(skillCounts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)

    // Find max for bar scaling
    const maxSkillCount = topSkills[0]?.[1] || 1

    // Project stats sorted by session count
    const projectStats = projects
      .map(p => ({
        name: p.displayName,
        fullName: p.name,
        sessions: p.sessions.length,
        activeCount: p.activeCount,
      }))
      .sort((a, b) => b.sessions - a.sessions)
      .slice(0, 5)

    const maxProjectSessions = projectStats[0]?.sessions || 1

    // Find earliest session
    const earliest = allSessions.reduce((min, s) => {
      const d = new Date(s.modifiedAt)
      return d < min ? d : min
    }, new Date())

    return {
      totalSessions: allSessions.length,
      totalProjects: projects.length,
      since: earliest.toLocaleDateString('en-US', { month: 'short', year: 'numeric' }),
      topSkills,
      maxSkillCount,
      projectStats,
      maxProjectSessions,
    }
  }, [projects])

  return (
    <div className="bg-white rounded-xl border border-gray-200 p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-2">
        <BarChart3 className="w-5 h-5 text-gray-400" />
        <h2 className="text-lg font-semibold text-gray-900">Your Usage</h2>
      </div>

      {/* Overview stats */}
      <div className="flex items-center gap-4 text-sm text-gray-600">
        <span className="tabular-nums font-medium">{stats.totalSessions}</span> sessions
        <span className="text-gray-300">·</span>
        <span className="tabular-nums font-medium">{stats.totalProjects}</span> projects
        <span className="text-gray-300">·</span>
        since {stats.since}
      </div>

      {/* Top skills */}
      {stats.topSkills.length > 0 && (
        <div>
          <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3 flex items-center gap-1.5">
            <Zap className="w-3.5 h-3.5" />
            Top Skills
          </h3>
          <div className="space-y-2">
            {stats.topSkills.map(([skill, count]) => (
              <button
                key={skill}
                onClick={() => onFilterClick(`skill:${skill.replace('/', '')}`)}
                className="w-full group"
              >
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="font-mono text-gray-700 group-hover:text-blue-600 transition-colors">
                    {skill}
                  </span>
                  <span className="tabular-nums text-gray-400">{count}</span>
                </div>
                <div className="h-1.5 bg-gray-100 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-[#7c9885] group-hover:bg-blue-500 transition-colors rounded-full"
                    style={{ width: `${(count / stats.maxSkillCount) * 100}%` }}
                  />
                </div>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Top projects */}
      <div>
        <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3 flex items-center gap-1.5">
          <FolderOpen className="w-3.5 h-3.5" />
          Most Active Projects
        </h3>
        <div className="space-y-2">
          {stats.projectStats.map((project) => (
            <button
              key={project.fullName}
              onClick={() => onFilterClick(`project:${project.name}`)}
              className="w-full group"
            >
              <div className="flex items-center justify-between text-sm mb-1">
                <span className="flex items-center gap-2">
                  <span className="text-gray-700 group-hover:text-blue-600 transition-colors">
                    {project.name}
                  </span>
                  {project.activeCount > 0 && (
                    <span className="flex items-center gap-1 text-xs text-green-600">
                      <span className="w-1.5 h-1.5 bg-green-500 rounded-full" />
                      {project.activeCount}
                    </span>
                  )}
                </span>
                <span className="tabular-nums text-gray-400">{project.sessions}</span>
              </div>
              <div className="h-1.5 bg-gray-100 rounded-full overflow-hidden">
                <div
                  className={cn(
                    "h-full rounded-full transition-colors",
                    project.activeCount > 0
                      ? "bg-green-400 group-hover:bg-green-500"
                      : "bg-gray-300 group-hover:bg-blue-500"
                  )}
                  style={{ width: `${(project.sessions / stats.maxProjectSessions) * 100}%` }}
                />
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
