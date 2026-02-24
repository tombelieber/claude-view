import { CalendarDays } from 'lucide-react'

export function ActivityPage() {
  return (
    <div className="h-full flex flex-col overflow-y-auto">
      <div className="px-6 pt-6 pb-4">
        <div className="flex items-center gap-2 mb-1">
          <CalendarDays className="w-5 h-5 text-blue-500" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Activity</h1>
        </div>
        <p className="text-sm text-gray-500 dark:text-gray-400">Where your Claude Code time goes</p>
      </div>
      <div className="px-6 pb-6 text-sm text-gray-400">Coming soon — stats, heatmap, project breakdown, daily timeline</div>
    </div>
  )
}
