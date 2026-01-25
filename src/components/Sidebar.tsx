import { useMemo } from 'react'
import { Link, useParams, useLocation, useNavigate } from 'react-router-dom'
import { FolderOpen, Pencil, Eye, Terminal } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface SidebarProps {
  projects: ProjectInfo[]
}

export function Sidebar({ projects }: SidebarProps) {
  const params = useParams()
  const location = useLocation()
  const navigate = useNavigate()

  // Determine selected project from URL
  const selectedProjectId = params.projectId ? decodeURIComponent(params.projectId) : null
  const selectedProject = projects.find(p => p.name === selectedProjectId)

  // Calculate per-project stats when a project is selected
  const projectStats = useMemo(() => {
    if (!selectedProject) return null

    const skillCounts = new Map<string, number>()
    const fileCounts = new Map<string, number>()
    let totalEdits = 0, totalReads = 0, totalBash = 0

    for (const session of selectedProject.sessions) {
      for (const skill of session.skillsUsed ?? []) {
        skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
      }
      for (const file of session.filesTouched ?? []) {
        fileCounts.set(file, (fileCounts.get(file) || 0) + 1)
      }
      const tc = session.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
      totalEdits += tc.edit + tc.write
      totalReads += tc.read
      totalBash += tc.bash
    }

    const maxTools = Math.max(totalEdits, totalReads, totalBash, 1)

    return {
      topSkills: Array.from(skillCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 5),
      topFiles: Array.from(fileCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 5),
      tools: {
        edits: totalEdits,
        reads: totalReads,
        bash: totalBash,
        maxTools,
      },
    }
  }, [selectedProject])

  const handleSkillClick = (skill: string) => {
    const query = selectedProject
      ? `project:${selectedProject.displayName} skill:${skill.replace('/', '')}`
      : `skill:${skill.replace('/', '')}`
    navigate(`/search?q=${encodeURIComponent(query)}`)
  }

  const handleFileClick = (file: string) => {
    const query = selectedProject
      ? `project:${selectedProject.displayName} path:${file}`
      : `path:${file}`
    navigate(`/search?q=${encodeURIComponent(query)}`)
  }

  return (
    <aside className="w-72 bg-gray-50/80 border-r border-gray-200 flex flex-col overflow-hidden">
      {/* Project List */}
      <div className="flex-1 overflow-y-auto py-2">
        {projects.map((project) => {
          const isSelected = selectedProjectId === project.name
          const parentPath = project.name.split('/').slice(0, -1).join('/')
          const hasActive = project.activeCount > 0

          return (
            <Link
              key={project.name}
              to={`/project/${encodeURIComponent(project.name)}`}
              className={cn(
                'w-full flex items-start gap-2.5 px-3 py-2 text-left transition-colors',
                isSelected
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-700 hover:bg-gray-200/70'
              )}
            >
              <FolderOpen className={cn(
                'w-4 h-4 flex-shrink-0 mt-0.5',
                isSelected ? 'text-white' : 'text-blue-400'
              )} />
              <div className="flex-1 min-w-0">
                <span className="truncate font-medium text-[13px] block">
                  {project.displayName}
                </span>
                {parentPath && (
                  <p className={cn(
                    'text-[11px] truncate mt-0.5',
                    isSelected ? 'text-blue-100' : 'text-gray-400'
                  )}>
                    {parentPath}
                  </p>
                )}
              </div>
              <div className="flex items-center gap-1.5 flex-shrink-0">
                {hasActive && (
                  <span className="flex items-center gap-1">
                    <span className={cn(
                      'w-1.5 h-1.5 rounded-full animate-pulse',
                      isSelected ? 'bg-green-300' : 'bg-green-500'
                    )} />
                    <span className={cn(
                      'text-xs tabular-nums',
                      isSelected ? 'text-green-200' : 'text-green-600'
                    )}>
                      {project.activeCount}
                    </span>
                  </span>
                )}
                <span className={cn(
                  'text-xs tabular-nums',
                  isSelected ? 'text-blue-100' : 'text-gray-400'
                )}>
                  {project.sessions.length}
                </span>
              </div>
            </Link>
          )
        })}
      </div>

      {/* Per-Project Stats Panel - Shows when project selected */}
      {projectStats && selectedProject && (
        <div className="border-t border-gray-200 p-3 space-y-4 bg-white">
          {/* Project Header */}
          <div>
            <h3 className="font-medium text-sm text-gray-900 truncate">
              {selectedProject.displayName}
            </h3>
            <p className="text-[11px] text-gray-400 truncate">
              {selectedProject.path}
            </p>
            <p className="text-xs text-gray-500 mt-1">
              {selectedProject.activeCount > 0 && (
                <span className="text-green-600">
                  ●{selectedProject.activeCount} active ·
                </span>
              )}
              {selectedProject.sessions.length} sessions
            </p>
          </div>

          {/* Skills */}
          {projectStats.topSkills.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-2">
                Skills
              </p>
              <div className="flex flex-wrap gap-1">
                {projectStats.topSkills.map(([skill, count]) => (
                  <button
                    key={skill}
                    onClick={() => handleSkillClick(skill)}
                    className="px-1.5 py-0.5 text-[11px] font-mono bg-gray-100 hover:bg-blue-500 hover:text-white text-gray-600 rounded transition-colors"
                  >
                    {skill} <span className="opacity-60">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Top Files */}
          {projectStats.topFiles.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-2">
                Top Files
              </p>
              <div className="space-y-0.5">
                {projectStats.topFiles.map(([file, count]) => (
                  <button
                    key={file}
                    onClick={() => handleFileClick(file)}
                    className="w-full flex items-center justify-between px-1.5 py-1 text-[11px] hover:bg-gray-100 rounded transition-colors text-left"
                  >
                    <span className="truncate text-gray-600 font-mono">{file}</span>
                    <span className="text-gray-400 tabular-nums ml-2">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Tool Usage Bars */}
          <div>
            <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-2">
              Tools
            </p>
            <div className="space-y-2">
              {[
                { label: 'Edit', value: projectStats.tools.edits, icon: Pencil, color: 'bg-blue-400' },
                { label: 'Read', value: projectStats.tools.reads, icon: Eye, color: 'bg-green-400' },
                { label: 'Bash', value: projectStats.tools.bash, icon: Terminal, color: 'bg-amber-400' },
              ].map(({ label, value, icon: Icon, color }) => (
                <div key={label} className="flex items-center gap-2">
                  <Icon className="w-3 h-3 text-gray-400" />
                  <span className="text-[11px] text-gray-600 w-8">{label}</span>
                  <div className="flex-1 h-1.5 bg-gray-100 rounded-full overflow-hidden">
                    <div
                      className={cn('h-full rounded-full transition-all', color)}
                      style={{ width: `${(value / projectStats.tools.maxTools) * 100}%` }}
                    />
                  </div>
                  <span className="text-[11px] text-gray-400 tabular-nums w-8 text-right">
                    {value}
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </aside>
  )
}
