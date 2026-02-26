import { useState } from 'react'
import { LayoutGrid, BarChart3, Table } from 'lucide-react'
import { CategoryTreemap } from './CategoryTreemap'
import { CategoryBarChart } from './CategoryBarChart'
import { CategoryTable } from './CategoryTable'
import type { CategoryNode } from '../../types/generated/CategoryNode'

type ViewMode = 'treemap' | 'bar' | 'table'

interface VisualizationProps {
  data: CategoryNode[]
  onCategoryClick: (categoryId: string) => void
  selectedCategory: string | null
}

const VIEW_OPTIONS: {
  value: ViewMode
  label: string
  icon: React.ElementType
}[] = [
  { value: 'treemap', label: 'Treemap', icon: LayoutGrid },
  { value: 'bar', label: 'Bar Chart', icon: BarChart3 },
  { value: 'table', label: 'Table', icon: Table },
]

export function CategoriesVisualization({
  data,
  onCategoryClick,
  selectedCategory,
}: VisualizationProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('treemap')

  return (
    <div className="space-y-4">
      {/* View Toggle */}
      <div className="flex items-center justify-end gap-1 p-1 bg-gray-100 dark:bg-gray-800 rounded-lg w-fit ml-auto">
        {VIEW_OPTIONS.map(({ value, label, icon: Icon }) => (
          <button
            key={value}
            onClick={() => setViewMode(value)}
            className={`
              flex items-center gap-2 px-3 py-1.5 rounded-md text-sm font-medium
              transition-colors duration-150 cursor-pointer
              ${
                viewMode === value
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                  : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
              }
            `}
            aria-pressed={viewMode === value}
          >
            <Icon className="w-4 h-4" />
            <span className="hidden sm:inline">{label}</span>
          </button>
        ))}
      </div>

      {/* Visualization */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
        {viewMode === 'treemap' && (
          <CategoryTreemap
            data={data}
            onCategoryClick={onCategoryClick}
            selectedCategory={selectedCategory}
          />
        )}
        {viewMode === 'bar' && (
          <CategoryBarChart
            data={data}
            onCategoryClick={onCategoryClick}
            selectedCategory={selectedCategory}
          />
        )}
        {viewMode === 'table' && (
          <CategoryTable
            data={data}
            onCategoryClick={onCategoryClick}
            selectedCategory={selectedCategory}
          />
        )}
      </div>
    </div>
  )
}
