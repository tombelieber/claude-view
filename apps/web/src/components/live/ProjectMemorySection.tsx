import {
  Brain,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FolderKanban,
  MessageSquareWarning,
  UserCircle,
} from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { type MemoryType, useMemoryIndex } from '../../hooks/use-memory'
import { cn } from '../../lib/utils'

// ── Type pill (compact for side panel) ──

const TYPE_ICON: Record<MemoryType, React.ComponentType<{ className?: string }>> = {
  user: UserCircle,
  feedback: MessageSquareWarning,
  project: FolderKanban,
  reference: ExternalLink,
}

const TYPE_COLOR: Record<MemoryType, string> = {
  user: 'text-violet-600 dark:text-violet-400',
  feedback: 'text-amber-600 dark:text-amber-400',
  project: 'text-blue-600 dark:text-blue-400',
  reference: 'text-emerald-600 dark:text-emerald-400',
}

function MemoryTypeBadge({ type }: { type: MemoryType }) {
  const Icon = TYPE_ICON[type]
  return <Icon className={cn('w-3 h-3 flex-shrink-0', TYPE_COLOR[type])} />
}

// ── Type priority for sorting (feedback first — most actionable) ──

function typePriority(t: MemoryType): number {
  switch (t) {
    case 'feedback':
      return 0
    case 'project':
      return 1
    case 'reference':
      return 2
    case 'user':
      return 3
  }
}

/**
 * Collapsible "Project Memory" section for the session detail overview tab.
 *
 * Shows top 3 memories (sorted by type priority), with a "Show all N" link
 * to `/memory?project={projectDir}`. Only renders if the project has memories.
 */
export function ProjectMemorySection({ projectDir }: { projectDir: string }) {
  const { data } = useMemoryIndex()
  const [expanded, setExpanded] = useState(false)

  if (!data || !projectDir) return null

  const group = data.projects.find((g) => g.projectDir === projectDir)
  if (!group || group.count === 0) return null

  // Sort by type priority
  const sorted = [...group.memories].sort(
    (a, b) => typePriority(a.memoryType) - typePriority(b.memoryType),
  )

  const preview = sorted.slice(0, 3)
  const hasMore = sorted.length > 3

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 w-full text-left cursor-pointer"
      >
        {expanded ? (
          <ChevronDown className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
        ) : (
          <ChevronRight className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
        )}
        <Brain className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
        <span className="text-xs font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
          Project Memory ({group.count})
        </span>
      </button>

      {expanded && (
        <div className="mt-2 space-y-1">
          {preview.map((m) => (
            <div key={m.relativePath} className="flex items-center gap-2 py-1 text-xs">
              <MemoryTypeBadge type={m.memoryType} />
              <span className="text-gray-700 dark:text-gray-300 truncate flex-1">{m.name}</span>
              <span className="text-gray-400 dark:text-gray-500 text-[10px] flex-shrink-0">
                {m.memoryType}
              </span>
            </div>
          ))}
          {hasMore && (
            <Link
              to={`/memory?project=${encodeURIComponent(projectDir)}`}
              className="block text-xs text-blue-600 dark:text-blue-400 hover:text-blue-500 dark:hover:text-blue-300 mt-1"
            >
              Show all {group.count} →
            </Link>
          )}
        </div>
      )}
    </div>
  )
}
