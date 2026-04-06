import {
  Brain,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FolderKanban,
  Loader2,
  MessageSquareWarning,
  Search,
  UserCircle,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { type MemoryEntry, type MemoryType, useMemoryIndex } from '../hooks/use-memory'
import { cn } from '../lib/utils'
import { MarkdownBody } from '../components/MarkdownBody'

// ── Type colors ──

const TYPE_STYLES: Record<
  MemoryType,
  { bg: string; text: string; icon: React.ComponentType<{ className?: string }> }
> = {
  user: {
    bg: 'bg-violet-100 dark:bg-violet-900/40',
    text: 'text-violet-700 dark:text-violet-300',
    icon: UserCircle,
  },
  feedback: {
    bg: 'bg-amber-100 dark:bg-amber-900/40',
    text: 'text-amber-700 dark:text-amber-300',
    icon: MessageSquareWarning,
  },
  project: {
    bg: 'bg-blue-100 dark:bg-blue-900/40',
    text: 'text-blue-700 dark:text-blue-300',
    icon: FolderKanban,
  },
  reference: {
    bg: 'bg-emerald-100 dark:bg-emerald-900/40',
    text: 'text-emerald-700 dark:text-emerald-300',
    icon: ExternalLink,
  },
}

const TYPE_LABELS: MemoryType[] = ['user', 'feedback', 'project', 'reference']

function MemoryTypePill({ type }: { type: MemoryType }) {
  const style = TYPE_STYLES[type]
  const Icon = style.icon
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs font-medium whitespace-nowrap',
        style.bg,
        style.text,
      )}
    >
      <Icon className="w-3 h-3" />
      {type}
    </span>
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`
  const kb = bytes / 1024
  if (kb < 100) return `${kb.toFixed(1)}K`
  return `${Math.round(kb)}K`
}

// ── Memory Row ──

function MemoryRow({
  memory,
  isSelected,
  onClick,
}: {
  memory: MemoryEntry
  isSelected: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'w-full flex items-center gap-3 px-3 py-2 text-left text-sm',
        'hover:bg-gray-100 dark:hover:bg-gray-800/50 rounded-lg transition-colors',
        isSelected && 'bg-blue-50 dark:bg-blue-950/30 border-l-2 border-blue-500',
      )}
    >
      <MemoryTypePill type={memory.memoryType} />
      <span className="flex-1 truncate text-gray-900 dark:text-gray-100 font-medium">
        {memory.name}
      </span>
      <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums flex-shrink-0">
        {formatBytes(memory.sizeBytes)}
      </span>
    </button>
  )
}

// ── Section Header ──

function SectionHeader({
  label,
  count,
  isOpen,
  onToggle,
}: {
  label: string
  count: number
  isOpen: boolean
  onToggle: () => void
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-50 dark:hover:bg-gray-800/30 rounded-lg transition-colors"
    >
      {isOpen ? (
        <ChevronDown className="w-3.5 h-3.5 text-gray-400" />
      ) : (
        <ChevronRight className="w-3.5 h-3.5 text-gray-400" />
      )}
      <span className="text-sm font-semibold text-gray-700 dark:text-gray-300">{label}</span>
      <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">{count}</span>
    </button>
  )
}

// ── Detail Panel ──

function MemoryDetail({ memory }: { memory: MemoryEntry }) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      <div className="px-5 py-4 border-b border-gray-100 dark:border-gray-800">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 mb-1.5">
          {memory.name}
        </h3>
        <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
          <MemoryTypePill type={memory.memoryType} />
          <span>·</span>
          <span>{memory.scope}</span>
          <span>·</span>
          <span className="tabular-nums">{formatBytes(memory.sizeBytes)}</span>
        </div>
        {memory.description && (
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">{memory.description}</p>
        )}
      </div>
      <div className="px-5 py-4 prose prose-sm dark:prose-invert max-w-none prose-p:my-1.5 prose-li:my-0.5 prose-headings:mt-3 prose-headings:mb-1.5 prose-code:text-xs prose-pre:text-xs">
        <MarkdownBody content={memory.body} />
      </div>
    </div>
  )
}

// ── Main Page ──

export function MemoryPage() {
  const { data, isLoading, error } = useMemoryIndex()
  const [searchParams] = useSearchParams()
  const projectParam = searchParams.get('project')
  const [selected, setSelected] = useState<MemoryEntry | null>(null)
  const [typeFilter, setTypeFilter] = useState<MemoryType | 'all'>('all')
  const [searchQuery, setSearchQuery] = useState('')
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set(['Global']))

  // Auto-expand project section when arriving from "Show all →" link
  useEffect(() => {
    if (projectParam && data) {
      setExpandedSections((prev) => {
        if (prev.has(projectParam)) return prev
        return new Set(prev).add(projectParam)
      })
    }
  }, [projectParam, data])

  const toggleSection = useCallback((key: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev)
      if (next.has(key)) {
        next.delete(key)
      } else {
        next.add(key)
      }
      return next
    })
  }, [])

  // Filter memories
  const filterMemories = useCallback(
    (memories: MemoryEntry[]) => {
      return memories.filter((m) => {
        if (typeFilter !== 'all' && m.memoryType !== typeFilter) return false
        if (searchQuery) {
          const q = searchQuery.toLowerCase()
          return (
            m.name.toLowerCase().includes(q) ||
            m.description.toLowerCase().includes(q) ||
            m.body.toLowerCase().includes(q)
          )
        }
        return true
      })
    },
    [typeFilter, searchQuery],
  )

  const filteredData = useMemo(() => {
    if (!data) return null
    const global = filterMemories(data.global)
    const projects = data.projects
      .map((group) => ({
        ...group,
        memories: filterMemories(group.memories),
        count: filterMemories(group.memories).length,
      }))
      .filter((g) => g.count > 0)

    return {
      global,
      projects,
      totalCount: global.length + projects.reduce((s, g) => s + g.count, 0),
    }
  }, [data, filterMemories])

  // Auto-expand sections that have search matches
  const prevSearchRef = useMemo(() => searchQuery, [searchQuery])
  useMemo(() => {
    if (searchQuery && filteredData) {
      const newExpanded = new Set<string>(['Global'])
      for (const group of filteredData.projects) {
        if (group.count > 0) {
          newExpanded.add(group.projectDir)
        }
      }
      setExpandedSections(newExpanded)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prevSearchRef])

  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center">
        <Loader2 className="w-6 h-6 animate-spin text-gray-400" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center text-red-500 text-sm">
        Failed to load memory: {error.message}
      </div>
    )
  }

  if (!data || data.totalCount === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-gray-400 dark:text-gray-500 gap-3">
        <Brain className="w-10 h-10" />
        <p className="text-sm">
          No memories yet. Claude Code will build memories as you work together.
        </p>
      </div>
    )
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-4xl mx-auto px-6 py-8">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3">
            <Brain className="w-5 h-5 text-gray-400 dark:text-gray-500" />
            <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Memory</h1>
            <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
              {filteredData?.totalCount ?? data.totalCount} memories
            </span>
          </div>
        </div>

        {/* Toolbar */}
        <div className="flex items-center gap-4 mb-6">
          {/* Search */}
          <div className="relative flex-1 max-w-sm">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search memories..."
              className="w-full pl-9 pr-3 py-1.5 text-sm border border-gray-200 dark:border-gray-700 rounded-md bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-400 focus:outline-none"
            />
          </div>

          {/* Type filter */}
          <div className="flex items-center gap-0.5 p-0.5 bg-gray-100 dark:bg-gray-800 rounded-md">
            <button
              type="button"
              onClick={() => setTypeFilter('all')}
              className={cn(
                'px-2.5 py-1 text-xs font-medium rounded transition-colors',
                typeFilter === 'all'
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
              )}
            >
              All
            </button>
            {TYPE_LABELS.map((t) => (
              <button
                key={t}
                type="button"
                onClick={() => setTypeFilter(t)}
                className={cn(
                  'px-2.5 py-1 text-xs font-medium rounded transition-colors',
                  typeFilter === t
                    ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                    : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
                )}
              >
                {t}
              </button>
            ))}
          </div>
        </div>

        {/* Content */}
        <div className="space-y-1">
          {/* Global section */}
          {filteredData && filteredData.global.length > 0 && (
            <div>
              <SectionHeader
                label="Global"
                count={filteredData.global.length}
                isOpen={expandedSections.has('Global')}
                onToggle={() => toggleSection('Global')}
              />
              {expandedSections.has('Global') && (
                <div className="ml-2">
                  {filteredData.global.map((m) => (
                    <MemoryRow
                      key={m.relativePath}
                      memory={m}
                      isSelected={selected?.relativePath === m.relativePath}
                      onClick={() =>
                        setSelected(selected?.relativePath === m.relativePath ? null : m)
                      }
                    />
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Project sections */}
          {filteredData?.projects.map((group) => (
            <div key={group.projectDir}>
              <SectionHeader
                label={`${group.displayName} (${group.count})`}
                count={group.count}
                isOpen={expandedSections.has(group.projectDir)}
                onToggle={() => toggleSection(group.projectDir)}
              />
              {expandedSections.has(group.projectDir) && (
                <div className="ml-2">
                  {group.memories.map((m) => (
                    <MemoryRow
                      key={m.relativePath}
                      memory={m}
                      isSelected={selected?.relativePath === m.relativePath}
                      onClick={() =>
                        setSelected(selected?.relativePath === m.relativePath ? null : m)
                      }
                    />
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>

        {/* Detail panel */}
        {selected && (
          <div className="mt-6">
            <MemoryDetail memory={selected} />
          </div>
        )}
      </div>
    </div>
  )
}
