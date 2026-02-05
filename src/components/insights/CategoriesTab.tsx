import { useState, useCallback, useMemo } from 'react'
import { useCategories } from '../../hooks/use-categories'
import { CategoryStatsSummary } from './CategoryStatsSummary'
import { CategoriesVisualization } from './CategoriesVisualization'
import { CategoryDrillDown } from './CategoryDrillDown'
import type { CategoryNode } from '../../types/generated/CategoryNode'
import type { TimeRange } from '../../hooks/use-insights'

interface CategoriesTabProps {
  timeRange: TimeRange
}

export function CategoriesTab({ timeRange }: CategoriesTabProps) {
  const [selectedCategoryId, setSelectedCategoryId] = useState<string | null>(null)

  const { data, isLoading, error } = useCategories({ timeRange })

  // Find selected category in tree
  const findCategory = useCallback(
    (id: string, nodes: CategoryNode[]): CategoryNode | null => {
      for (const node of nodes) {
        if (node.id === id) return node
        if (node.children?.length) {
          const found = findCategory(id, node.children)
          if (found) return found
        }
      }
      return null
    },
    [],
  )

  const selectedCategory = useMemo(
    () =>
      selectedCategoryId && data
        ? findCategory(selectedCategoryId, data.categories)
        : null,
    [selectedCategoryId, data, findCategory],
  )

  // Find parent category for breadcrumb
  const parentCategory = useMemo(() => {
    if (!selectedCategoryId || !data) return null
    const parts = selectedCategoryId.split('/')
    if (parts.length <= 1) return null
    const parentId = parts.slice(0, -1).join('/')
    return findCategory(parentId, data.categories)
  }, [selectedCategoryId, data, findCategory])

  const handleCategoryClick = useCallback((categoryId: string) => {
    setSelectedCategoryId(categoryId || null)
  }, [])

  const handleBack = useCallback(() => {
    if (!selectedCategoryId) return
    const parts = selectedCategoryId.split('/')
    if (parts.length <= 1) {
      setSelectedCategoryId(null)
    } else {
      setSelectedCategoryId(parts.slice(0, -1).join('/'))
    }
  }, [selectedCategoryId])

  if (isLoading) {
    return <CategoriesTabSkeleton />
  }

  if (error) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600 dark:text-red-400">
          Failed to load category data
        </p>
        <button
          onClick={() => window.location.reload()}
          className="mt-2 text-sm text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
        >
          Retry
        </button>
      </div>
    )
  }

  if (!data) return null

  // Show drill-down view if category selected
  if (selectedCategory) {
    return (
      <CategoryDrillDown
        category={selectedCategory}
        parentCategory={parentCategory ?? undefined}
        overallAverages={data.overallAverages}
        onBack={handleBack}
        onDrillDown={handleCategoryClick}
      />
    )
  }

  // Show overview
  return (
    <div className="space-y-6">
      {/* Quick Stats */}
      <CategoryStatsSummary
        breakdown={data.breakdown}
        onCategoryClick={handleCategoryClick}
      />

      {/* Visualization */}
      <CategoriesVisualization
        data={data.categories}
        onCategoryClick={handleCategoryClick}
        selectedCategory={selectedCategoryId}
      />
    </div>
  )
}

function CategoriesTabSkeleton() {
  return (
    <div className="animate-pulse space-y-6">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        {[0, 1, 2, 3].map((i) => (
          <div
            key={i}
            className="h-32 rounded-lg bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700"
          />
        ))}
      </div>
      <div className="h-[400px] rounded-lg bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700" />
    </div>
  )
}
