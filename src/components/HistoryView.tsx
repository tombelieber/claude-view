// src/components/HistoryView.tsx

import { useState, useMemo, useRef, useEffect } from 'react'
import { Link, useOutletContext } from 'react-router-dom'
import { Search, X, FolderOpen } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { SessionCard } from './SessionCard'
import { ActivitySparkline } from './ActivitySparkline'
import { groupSessionsByDate } from '../lib/date-groups'

interface OutletContext {
  projects: ProjectInfo[]
}

type TimeFilter = 'all' | 'today' | '7d' | '30d'

export function HistoryView() {
  const { projects } = useOutletContext<OutletContext>()
  const [searchText, setSearchText] = useState('')
  const [selectedProjects, setSelectedProjects] = useState<Set<string>>(new Set())
  const [timeFilter, setTimeFilter] = useState<TimeFilter>('all')
  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const [showProjectFilter, setShowProjectFilter] = useState(false)
  const searchRef = useRef<HTMLInputElement>(null)
  const filterRef = useRef<HTMLDivElement>(null)

  // Focus search on mount
  useEffect(() => {
    searchRef.current?.focus()
  }, [])

  // Close project filter on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (filterRef.current && !filterRef.current.contains(e.target as Node)) {
        setShowProjectFilter(false)
      }
    }
    if (showProjectFilter) {
      document.addEventListener('mousedown', handleClick)
      return () => document.removeEventListener('mousedown', handleClick)
    }
  }, [showProjectFilter])

  // Map project names to display names
  const projectDisplayNames = useMemo(() => {
    const map = new Map<string, string>()
    for (const p of projects) {
      for (const s of p.sessions) {
        map.set(s.id, p.displayName)
      }
    }
    return map
  }, [projects])

  // Flatten all sessions
  const allSessions = useMemo(() => {
    return projects
      .flatMap(p => p.sessions)
      .sort((a, b) => b.modifiedAt - a.modifiedAt)
  }, [projects])

  // Apply filters
  const filteredSessions = useMemo(() => {
    const now = Math.floor(Date.now() / 1000)
    const cutoffs: Record<TimeFilter, number> = {
      all: 0,
      today: now - 86400,
      '7d': now - 7 * 86400,
      '30d': now - 30 * 86400,
    }
    const cutoff = cutoffs[timeFilter]
    const query = searchText.toLowerCase().trim()

    return allSessions.filter(s => {
      // Time filter
      if (cutoff > 0 && s.modifiedAt < cutoff) return false

      // Project filter
      if (selectedProjects.size > 0 && !selectedProjects.has(s.project)) return false

      // Date filter (from sparkline click)
      if (selectedDate) {
        const d = new Date(s.modifiedAt * 1000)
        const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
        if (key !== selectedDate) return false
      }

      // Text search
      if (query) {
        const haystack = [
          s.preview,
          s.lastMessage,
          ...(s.filesTouched ?? []),
          ...(s.skillsUsed ?? []),
          s.project,
        ].join(' ').toLowerCase()
        return haystack.includes(query)
      }

      return true
    })
  }, [allSessions, searchText, selectedProjects, timeFilter, selectedDate])

  const isFiltered = searchText || selectedProjects.size > 0 || timeFilter !== 'all' || selectedDate
  const groups = groupSessionsByDate(filteredSessions)

  // Project list sorted by session count
  const sortedProjects = useMemo(() => {
    return [...projects].sort((a, b) => b.sessions.length - a.sessions.length)
  }, [projects])

  function toggleProject(name: string) {
    setSelectedProjects(prev => {
      const next = new Set(prev)
      if (next.has(name)) {
        next.delete(name)
      } else {
        next.add(name)
      }
      return next
    })
  }

  function clearAll() {
    setSearchText('')
    setSelectedProjects(new Set())
    setTimeFilter('all')
    setSelectedDate(null)
  }

  const timeOptions: { value: TimeFilter; label: string }[] = [
    { value: 'all', label: 'All time' },
    { value: 'today', label: 'Today' },
    { value: '7d', label: '7 days' },
    { value: '30d', label: '30 days' },
  ]

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-6 py-5">

        {/* ── Activity sparkline chart ── */}
        <ActivitySparkline
          sessions={allSessions}
          selectedDate={selectedDate}
          onDateSelect={setSelectedDate}
        />

        {/* ── Search + Filters bar ── */}
        <div className="mt-5 space-y-3">
          {/* Search input */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              ref={searchRef}
              type="text"
              value={searchText}
              onChange={e => setSearchText(e.target.value)}
              placeholder="Search sessions, files, skills..."
              className="w-full pl-9 pr-9 py-2.5 text-sm bg-gray-50 border border-gray-200 rounded-lg outline-none transition-colors focus:bg-white focus:border-gray-400 focus:ring-1 focus:ring-gray-400/20 placeholder:text-gray-400"
            />
            {searchText && (
              <button
                onClick={() => setSearchText('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>

          {/* Filter row: time + project */}
          <div className="flex items-center gap-2 flex-wrap">
            {/* Time filters */}
            <div className="flex items-center gap-0.5 p-0.5 bg-gray-100 rounded-md">
              {timeOptions.map(opt => (
                <button
                  key={opt.value}
                  onClick={() => setTimeFilter(opt.value)}
                  className={`px-2.5 py-1 text-xs font-medium rounded-md transition-all ${
                    timeFilter === opt.value
                      ? 'bg-white text-gray-900 shadow-sm'
                      : 'text-gray-500 hover:text-gray-700'
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>

            {/* Project filter dropdown */}
            <div className="relative" ref={filterRef}>
              <button
                onClick={() => setShowProjectFilter(!showProjectFilter)}
                className={`inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md border transition-all ${
                  selectedProjects.size > 0
                    ? 'bg-blue-50 border-blue-200 text-blue-700'
                    : 'bg-white border-gray-200 text-gray-600 hover:border-gray-300'
                }`}
              >
                <FolderOpen className="w-3.5 h-3.5" />
                {selectedProjects.size > 0
                  ? `${selectedProjects.size} project${selectedProjects.size > 1 ? 's' : ''}`
                  : 'Projects'}
              </button>

              {showProjectFilter && (
                <div className="absolute top-full left-0 mt-1.5 w-60 bg-white border border-gray-200 rounded-lg shadow-lg z-50 py-1 max-h-64 overflow-y-auto">
                  {selectedProjects.size > 0 && (
                    <button
                      onClick={() => setSelectedProjects(new Set())}
                      className="w-full text-left px-3 py-1.5 text-xs text-gray-500 hover:bg-gray-50 border-b border-gray-100"
                    >
                      Clear selection
                    </button>
                  )}
                  {sortedProjects.map(p => {
                    const checked = selectedProjects.has(p.name)
                    return (
                      <button
                        key={p.name}
                        onClick={() => toggleProject(p.name)}
                        className="w-full flex items-center gap-2.5 px-3 py-2 text-left hover:bg-gray-50 transition-colors"
                      >
                        <div className={`w-3.5 h-3.5 rounded border flex-shrink-0 flex items-center justify-center transition-colors ${
                          checked ? 'bg-blue-500 border-blue-500' : 'border-gray-300'
                        }`}>
                          {checked && (
                            <svg className="w-2.5 h-2.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                            </svg>
                          )}
                        </div>
                        <span className="text-sm text-gray-700 truncate flex-1">{p.displayName}</span>
                        <span className="text-xs text-gray-400 tabular-nums">{p.sessions.length}</span>
                      </button>
                    )
                  })}
                </div>
              )}
            </div>

            {/* Active filter summary */}
            {isFiltered && (
              <>
                <div className="h-4 w-px bg-gray-200" />
                <span className="text-xs text-gray-500 tabular-nums">
                  {filteredSessions.length} of {allSessions.length}
                </span>
                <button
                  onClick={clearAll}
                  className="text-xs text-gray-400 hover:text-gray-600 underline underline-offset-2"
                >
                  Clear all
                </button>
              </>
            )}
          </div>
        </div>

        {/* ── Session List ── */}
        <div className="mt-5">
          {filteredSessions.length > 0 ? (
            <div>
              {groups.map(group => (
                <div key={group.label}>
                  {/* Date header */}
                  <div className="sticky top-0 z-10 bg-white/95 backdrop-blur-sm py-2 flex items-center gap-3">
                    <span className="text-[13px] font-semibold text-gray-500 tracking-tight whitespace-nowrap">
                      {group.label}
                    </span>
                    <div className="flex-1 h-px bg-gray-150" style={{ backgroundColor: '#e8e8e8' }} />
                    <span className="text-[11px] text-gray-400 tabular-nums whitespace-nowrap">
                      {group.sessions.length}
                    </span>
                  </div>

                  {/* Cards */}
                  <div className="space-y-1.5 pb-3">
                    {group.sessions.map(session => (
                      <Link
                        key={session.id}
                        to={`/session/${encodeURIComponent(session.project)}/${session.id}`}
                        className="block"
                      >
                        <SessionCard
                          session={session}
                          isSelected={false}
                          onClick={() => {}}
                          projectDisplayName={projectDisplayNames.get(session.id)}
                        />
                      </Link>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-center py-16">
              <div className="inline-flex items-center justify-center w-12 h-12 rounded-full bg-gray-100 mb-3">
                <Search className="w-5 h-5 text-gray-400" />
              </div>
              <p className="text-sm font-medium text-gray-900">No sessions found</p>
              <p className="text-sm text-gray-500 mt-1">
                {isFiltered ? 'Try adjusting your filters' : 'No session history yet'}
              </p>
              {isFiltered && (
                <button
                  onClick={clearAll}
                  className="mt-3 text-sm text-blue-600 hover:text-blue-700"
                >
                  Clear filters
                </button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
