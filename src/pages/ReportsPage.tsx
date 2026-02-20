import { useState } from 'react'
import { useSmartDefaults } from '../hooks/use-smart-defaults'
import { useReports } from '../hooks/use-reports'
import { ReportCard } from '../components/reports/ReportCard'
import { ReportContent } from '../components/reports/ReportContent'
import { ReportHistory } from '../components/reports/ReportHistory'
import type { ReportRow } from '../types/generated/ReportRow'

export function ReportsPage() {
  const defaults = useSmartDefaults()
  const { data: reports } = useReports()
  const [selectedReport, setSelectedReport] = useState<ReportRow | null>(null)

  // Find existing reports matching the card dates
  const findExisting = (dateStart: string, dateEnd: string) =>
    reports?.find(r => r.dateStart === dateStart && r.dateEnd === dateEnd)

  return (
    <div className="max-w-3xl mx-auto py-6 px-4 space-y-6">
      <div>
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Work Reports</h1>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
          AI-generated summaries of your Claude Code sessions
        </p>
      </div>

      {/* Primary + Secondary cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <ReportCard
          label={defaults.primary.label}
          dateStart={defaults.primary.dateStart}
          dateEnd={defaults.primary.dateEnd}
          type={defaults.primary.type}
          startTs={defaults.primary.startTs}
          endTs={defaults.primary.endTs}
          existingReport={findExisting(defaults.primary.dateStart, defaults.primary.dateEnd)}
        />
        <ReportCard
          label={defaults.secondary.label}
          dateStart={defaults.secondary.dateStart}
          dateEnd={defaults.secondary.dateEnd}
          type={defaults.secondary.type}
          startTs={defaults.secondary.startTs}
          endTs={defaults.secondary.endTs}
          existingReport={findExisting(defaults.secondary.dateStart, defaults.secondary.dateEnd)}
        />
      </div>

      {/* Selected report viewer */}
      {selectedReport && (
        <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-5">
          <div className="flex items-center justify-between mb-3">
            <div>
              <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                {selectedReport.reportType === 'daily' ? 'Daily' : 'Weekly'} Report
              </h3>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                {selectedReport.dateStart === selectedReport.dateEnd
                  ? selectedReport.dateStart
                  : `${selectedReport.dateStart} \u2014 ${selectedReport.dateEnd}`}
              </p>
            </div>
            <button
              type="button"
              onClick={() => setSelectedReport(null)}
              className="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
            >
              Close
            </button>
          </div>
          <ReportContent markdown={selectedReport.contentMd} />
        </div>
      )}

      {/* Report History */}
      <div>
        <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100 mb-3">History</h2>
        <ReportHistory onSelect={setSelectedReport} selectedId={selectedReport?.id} />
      </div>
    </div>
  )
}
