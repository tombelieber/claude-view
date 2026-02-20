import { Trash2 } from 'lucide-react'
import { useReports } from '../../hooks/use-reports'
import type { ReportRow } from '../../types/generated/ReportRow'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'

interface ReportHistoryProps {
  onSelect: (report: ReportRow) => void
  selectedId?: number
}

function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr + 'Z') // SQLite datetime is UTC
  const diff = (Date.now() - date.getTime()) / 1000
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

export function ReportHistory({ onSelect, selectedId }: ReportHistoryProps) {
  const { data: reports, isLoading } = useReports()
  const queryClient = useQueryClient()

  const handleDelete = useCallback(async (id: number, e: React.MouseEvent) => {
    e.stopPropagation()
    if (!window.confirm('Delete this report?')) return
    const res = await fetch(`/api/reports/${id}`, { method: 'DELETE' })
    if (res.ok) {
      queryClient.invalidateQueries({ queryKey: ['reports'] })
    }
  }, [queryClient])

  if (isLoading) {
    return (
      <div className="space-y-2">
        {[1, 2, 3].map(i => (
          <div key={i} className="h-12 bg-gray-100 dark:bg-gray-800 rounded-md animate-pulse" />
        ))}
      </div>
    )
  }

  if (!reports || reports.length === 0) {
    return (
      <p className="text-sm text-gray-400 dark:text-gray-500 text-center py-4">
        No reports generated yet
      </p>
    )
  }

  return (
    <div className="space-y-1">
      {reports.map(report => (
        <button
          key={report.id}
          type="button"
          onClick={() => onSelect(report)}
          className={`w-full flex items-center justify-between px-3 py-2 rounded-md text-left transition-colors ${
            selectedId === report.id
              ? 'bg-blue-50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800'
              : 'hover:bg-gray-50 dark:hover:bg-gray-800/50 border border-transparent'
          }`}
        >
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-xs font-medium px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400">
                {report.reportType}
              </span>
              <span className="text-xs text-gray-500 dark:text-gray-400">
                {report.dateStart === report.dateEnd ? report.dateStart : `${report.dateStart} \u2014 ${report.dateEnd}`}
              </span>
            </div>
            <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5 truncate">
              {report.sessionCount} sessions &middot; {formatRelativeTime(report.createdAt)}
            </p>
          </div>
          <button
            type="button"
            onClick={(e) => handleDelete(report.id, e)}
            className="p-1 rounded text-gray-300 dark:text-gray-600 hover:text-red-500 dark:hover:text-red-400 transition-colors flex-shrink-0"
            title="Delete report"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </button>
      ))}
    </div>
  )
}
