// src/components/HistoryView.tsx

import { useState, useMemo } from 'react'
import { useOutletContext } from 'react-router-dom'
import { Clock, X } from 'lucide-react'
import type { DateRange } from 'react-day-picker'
import type { ProjectInfo } from '../hooks/use-projects'
import { ActivityCalendar } from './ActivityCalendar'
import { DateGroupedList } from './DateGroupedList'

interface OutletContext {
  projects: ProjectInfo[]
}

export function HistoryView() {
  const { projects } = useOutletContext<OutletContext>()
  const [selectedRange, setSelectedRange] = useState<DateRange | undefined>()

  // Flatten all sessions, sort newest first
  const allSessions = useMemo(() => {
    return projects
      .flatMap(p => p.sessions)
      .sort((a, b) => b.modifiedAt - a.modifiedAt)
  }, [projects])

  // Filter by selected range
  const filteredSessions = useMemo(() => {
    if (!selectedRange?.from) return allSessions
    const from = new Date(selectedRange.from)
    from.setHours(0, 0, 0, 0)
    const fromTs = from.getTime() / 1000

    let toTs: number
    if (selectedRange.to) {
      const to = new Date(selectedRange.to)
      to.setHours(23, 59, 59, 999)
      toTs = to.getTime() / 1000
    } else {
      // Single day selected
      const to = new Date(selectedRange.from)
      to.setHours(23, 59, 59, 999)
      toTs = to.getTime() / 1000
    }

    return allSessions.filter(s => s.modifiedAt >= fromTs && s.modifiedAt <= toTs)
  }, [allSessions, selectedRange])

  const isFiltered = selectedRange?.from != null

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto">
        {/* Header */}
        <div className="flex items-center gap-2 mb-6">
          <Clock className="w-5 h-5 text-gray-400" />
          <h1 className="text-xl font-semibold text-gray-900">History</h1>
        </div>

        {/* Calendar Heatmap */}
        <div className="bg-white rounded-xl border border-gray-200 p-6 mb-6">
          <ActivityCalendar
            sessions={allSessions}
            selectedRange={selectedRange}
            onRangeChange={setSelectedRange}
            totalProjects={projects.length}
          />

          {isFiltered && (
            <button
              onClick={() => setSelectedRange(undefined)}
              className="mt-3 inline-flex items-center gap-1 px-2.5 py-1 text-xs font-medium text-gray-600 bg-gray-100 hover:bg-gray-200 rounded-full transition-colors"
            >
              <X className="w-3 h-3" />
              Clear filter
            </button>
          )}
        </div>

        {/* Session List */}
        {filteredSessions.length > 0 ? (
          <DateGroupedList sessions={filteredSessions} showProjectBadge />
        ) : (
          <div className="text-center py-12 text-gray-500">
            <p className="font-medium">No sessions found</p>
            {isFiltered && (
              <p className="text-sm mt-1">Try selecting a different date range</p>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
