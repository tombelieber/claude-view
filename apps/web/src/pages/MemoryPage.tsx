import {
  Brain,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FileText,
  FolderKanban,
  Loader2,
  MessageSquareWarning,
  Search,
  UserCircle,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
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
    bg: 'bg-violet-50 dark:bg-violet-950/30',
    text: 'text-violet-600 dark:text-violet-400',
    icon: UserCircle,
  },
  feedback: {
    bg: 'bg-amber-50 dark:bg-amber-950/30',
    text: 'text-amber-600 dark:text-amber-400',
    icon: MessageSquareWarning,
  },
  project: {
    bg: 'bg-blue-50 dark:bg-blue-950/30',
    text: 'text-blue-600 dark:text-blue-400',
    icon: FolderKanban,
  },
  reference: {
    bg: 'bg-emerald-50 dark:bg-emerald-950/30',
    text: 'text-emerald-600 dark:text-emerald-400',
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
        'inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium capitalize flex-shrink-0',
        style.bg,
        style.text,
      )}
    >
      <Icon className="w-3 h-3" />
      {type}
    </span>
  )
}

// ── Memory Row (left panel) ──

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
        'w-full flex items-center gap-2 px-3 py-2 text-left text-[13px] rounded-md transition-colors',
        'hover:bg-gray-100 dark:hover:bg-gray-800/60',
        isSelected
          ? 'bg-apple-blue/8 dark:bg-apple-blue/15 text-apple-blue font-medium'
          : 'text-gray-700 dark:text-gray-300',
      )}
    >
      <MemoryTypePill type={memory.memoryType} />
      <span className="flex-1 truncate">{memory.name}</span>
    </button>
  )
}

// ── Section Header (left panel) ──

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
  const Chevron = isOpen ? ChevronDown : ChevronRight
  return (
    <button
      type="button"
      onClick={onToggle}
      className="w-full flex items-center gap-1.5 px-3 py-1.5 text-left group"
    >
      <Chevron className="w-3 h-3 text-gray-400 dark:text-gray-500" />
      <span className="text-[11px] font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-[0.04em]">
        {label}
      </span>
      <span className="text-[10px] text-gray-400 dark:text-gray-500 tabular-nums">{count}</span>
    </button>
  )
}

// ── Detail Panel (right panel) ──

function MemoryDetail({ memory }: { memory: MemoryEntry }) {
  const style = TYPE_STYLES[memory.memoryType]
  const Icon = style.icon
  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-6 py-5 border-b border-gray-200/60 dark:border-gray-700/60 flex-shrink-0">
        <div className="flex items-start gap-3">
          <div className={cn('p-2 rounded-lg', style.bg)}>
            <Icon className={cn('w-4 h-4', style.text)} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100 leading-tight">
              {memory.name}
            </h2>
            <div className="flex items-center gap-1.5 mt-1.5 text-[11px] text-gray-400 dark:text-gray-500">
              <MemoryTypePill type={memory.memoryType} />
              <span>·</span>
              <span>{memory.scope}</span>
              <span>·</span>
              <span className="font-mono">{memory.filename}</span>
            </div>
            {memory.description && (
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-2 leading-relaxed">
                {memory.description}
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-6 py-5">
        <div className="prose prose-sm dark:prose-invert max-w-none prose-p:my-1.5 prose-li:my-0.5 prose-headings:mt-4 prose-headings:mb-2 prose-code:text-xs prose-pre:text-xs prose-pre:bg-gray-50 prose-pre:dark:bg-gray-800/50 prose-a:text-apple-blue prose-a:no-underline hover:prose-a:underline">
          <MarkdownBody content={memory.body} />
        </div>
      </div>
    </div>
  )
}

function EmptyDetail() {
  return (
    <div className="h-full flex flex-col items-center justify-center text-gray-400 dark:text-gray-500 gap-2 px-8">
      <FileText className="w-8 h-8 opacity-30" />
      <p className="text-[13px] text-center">Select a memory to view its content</p>
    </div>
  )
}

// ── Main Page ──

const MIN_PANEL_WIDTH = 240
const MAX_PANEL_WIDTH = 600
const DEFAULT_PANEL_WIDTH = 320

export function MemoryPage() {
  const { data, isLoading, error } = useMemoryIndex()
  const [searchParams] = useSearchParams()
  const projectParam = searchParams.get('project')
  const [selected, setSelected] = useState<MemoryEntry | null>(null)
  const [typeFilter, setTypeFilter] = useState<MemoryType | 'all'>('all')
  const [searchQuery, setSearchQuery] = useState('')
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set(['Global']))
  const [panelWidth, setPanelWidth] = useState(DEFAULT_PANEL_WIDTH)
  const [isResizing, setIsResizing] = useState(false)
  const widthRef = useRef(DEFAULT_PANEL_WIDTH)

  // Keep ref in sync
  useEffect(() => {
    widthRef.current = panelWidth
  }, [panelWidth])

  const handleResizeStart = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault()
    setIsResizing(true)
    const startX = e.clientX
    const startW = widthRef.current

    const onMove = (ev: PointerEvent) => {
      const delta = ev.clientX - startX
      const newWidth = Math.round(
        Math.max(MIN_PANEL_WIDTH, Math.min(MAX_PANEL_WIDTH, startW + delta)),
      )
      widthRef.current = newWidth
      setPanelWidth(newWidth)
    }

    const onUp = () => {
      setIsResizing(false)
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }

    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }, [])

  // Auto-expand project section when arriving from "Show all" link
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
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }, [])

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

  // Compute type counts from unfiltered data (search-filtered but not type-filtered)
  const typeCounts = useMemo(() => {
    if (!data) return { all: 0, user: 0, feedback: 0, project: 0, reference: 0 }

    const countByType = (memories: MemoryEntry[]) => {
      // Apply search filter only, not type filter
      const searched = searchQuery
        ? memories.filter((m) => {
            const q = searchQuery.toLowerCase()
            return (
              m.name.toLowerCase().includes(q) ||
              m.description.toLowerCase().includes(q) ||
              m.body.toLowerCase().includes(q)
            )
          })
        : memories

      const counts = { user: 0, feedback: 0, project: 0, reference: 0 }
      for (const m of searched) {
        counts[m.memoryType]++
      }
      return counts
    }

    const allMemories = [...data.global, ...data.projects.flatMap((g) => g.memories)]
    const counts = countByType(allMemories)
    const total = counts.user + counts.feedback + counts.project + counts.reference

    return { all: total, ...counts }
  }, [data, searchQuery])

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

  // Auto-expand sections that have search matches when query changes
  useEffect(() => {
    if (searchQuery && filteredData) {
      const newExpanded = new Set<string>(['Global'])
      for (const group of filteredData.projects) {
        if (group.count > 0) newExpanded.add(group.projectDir)
      }
      setExpandedSections(newExpanded)
    }
    // Only react to search query changes, not filteredData recalculations
    // biome-ignore lint/correctness/useExhaustiveDependencies: intentional — expand on search change only
  }, [searchQuery])

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
    <div className={cn('h-full flex bg-apple-bg dark:bg-gray-950', isResizing && 'select-none')}>
      {/* ── Left Panel: Memory List ── */}
      <div
        className="relative flex-shrink-0 border-r border-gray-200/80 dark:border-gray-800 flex flex-col bg-white/60 dark:bg-gray-900/40"
        style={{ width: panelWidth }}
      >
        {/* Header */}
        <div className="px-4 pt-5 pb-3 flex-shrink-0">
          <div className="flex items-center gap-2.5 mb-0.5">
            <Brain className="w-4 h-4 text-gray-400 dark:text-gray-500" />
            <h1 className="text-base font-semibold text-gray-900 dark:text-gray-100">Memory</h1>
            <span className="text-[11px] text-gray-400 dark:text-gray-500 tabular-nums ml-auto">
              {filteredData?.totalCount ?? data.totalCount}
            </span>
          </div>
        </div>

        {/* Search */}
        <div className="px-3 pb-2 flex-shrink-0">
          <div className="relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search..."
              className="w-full pl-8 pr-3 py-1.5 text-[13px] border border-gray-200 dark:border-gray-700 rounded-md bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 placeholder:text-gray-400 focus:ring-1 focus:ring-apple-blue/40 focus:border-apple-blue/40 focus:outline-none"
            />
          </div>
        </div>

        {/* Type filter pills with counts */}
        <div className="px-3 pb-3 flex-shrink-0">
          <div className="flex items-center gap-1 flex-wrap">
            <button
              type="button"
              onClick={() => setTypeFilter('all')}
              className={cn(
                'px-2 py-0.5 text-[11px] font-medium rounded-full transition-colors inline-flex items-center gap-1',
                typeFilter === 'all'
                  ? 'bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700',
              )}
            >
              All
              <span className="tabular-nums">{typeCounts.all}</span>
            </button>
            {TYPE_LABELS.map((t) => {
              const style = TYPE_STYLES[t]
              const count = typeCounts[t]
              return (
                <button
                  key={t}
                  type="button"
                  onClick={() => setTypeFilter(t)}
                  className={cn(
                    'px-2 py-0.5 text-[11px] font-medium rounded-full transition-colors capitalize inline-flex items-center gap-1',
                    typeFilter === t
                      ? cn(style.bg, style.text)
                      : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700',
                  )}
                >
                  {t}
                  <span className="tabular-nums">{count}</span>
                </button>
              )
            })}
          </div>
        </div>

        {/* Divider */}
        <div className="border-t border-gray-200/60 dark:border-gray-800" />

        {/* Scrollable list */}
        <div className="flex-1 overflow-y-auto px-1.5 py-2 space-y-0.5">
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
                <div className="ml-1">
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
                label={group.displayName}
                count={group.count}
                isOpen={expandedSections.has(group.projectDir)}
                onToggle={() => toggleSection(group.projectDir)}
              />
              {expandedSections.has(group.projectDir) && (
                <div className="ml-1">
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

        {/* Resize handle (right edge) */}
        <div
          onPointerDown={handleResizeStart}
          className="absolute top-0 right-0 w-1.5 h-full cursor-col-resize z-10 group"
        >
          <div className="w-px h-full mx-auto bg-transparent group-hover:bg-blue-500/40 group-active:bg-blue-500/60 transition-colors" />
        </div>
      </div>

      {/* ── Right Panel: Detail ── */}
      <div className="flex-1 min-w-0 bg-white dark:bg-gray-900">
        {selected ? <MemoryDetail memory={selected} /> : <EmptyDetail />}
      </div>
    </div>
  )
}
