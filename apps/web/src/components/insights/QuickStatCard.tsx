interface QuickStatCardProps {
  title: string
  icon: React.ReactNode
  children: React.ReactNode
  isLoading?: boolean
}

function QuickStatSkeleton() {
  return (
    <div className="animate-pulse">
      <div className="h-8 w-16 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
      <div className="h-3 w-24 bg-gray-200 dark:bg-gray-700 rounded mb-3" />
      <div className="h-3 w-32 bg-gray-200 dark:bg-gray-700 rounded" />
    </div>
  )
}

export function QuickStatCard({ title, icon, children, isLoading }: QuickStatCardProps) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5">
      <div className="flex items-center gap-2 mb-4">
        <span className="text-gray-400">{icon}</span>
        <h3 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          {title}
        </h3>
      </div>
      {isLoading ? <QuickStatSkeleton /> : children}
    </div>
  )
}
