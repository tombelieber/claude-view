import { useState, useMemo, useCallback } from 'react'
import { ChevronRight, ChevronDown } from 'lucide-react'
import type { CategoryNode } from '../../types/generated/CategoryNode'

interface TableProps {
  data: CategoryNode[]
  onCategoryClick: (categoryId: string) => void
  selectedCategory: string | null
}

type SortField = 'name' | 'count' | 'percentage' | 'avgReeditRate' | 'avgDuration' | 'commitRate'
type SortDir = 'asc' | 'desc'

function formatDuration(seconds: number): string {
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = Math.floor(minutes / 60)
  const remaining = minutes % 60
  return remaining > 0 ? `${hours}h ${remaining}m` : `${hours}h`
}

export function CategoryTable({
  data,
  onCategoryClick,
  selectedCategory,
}: TableProps) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set())
  const [sortField, setSortField] = useState<SortField>('count')
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }, [])

  const handleSort = useCallback(
    (field: SortField) => {
      if (field === sortField) {
        setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
      } else {
        setSortField(field)
        setSortDir('desc')
      }
    },
    [sortField],
  )

  // Sort the L1 categories
  const sortedData = useMemo(() => {
    const sorted = [...data]
    sorted.sort((a, b) => {
      const av = a[sortField as keyof CategoryNode]
      const bv = b[sortField as keyof CategoryNode]
      if (typeof av === 'number' && typeof bv === 'number') {
        return sortDir === 'asc' ? av - bv : bv - av
      }
      if (typeof av === 'string' && typeof bv === 'string') {
        return sortDir === 'asc' ? av.localeCompare(bv) : bv.localeCompare(av)
      }
      return 0
    })
    return sorted
  }, [data, sortField, sortDir])

  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center h-[400px] text-gray-400 dark:text-gray-500">
        No category data available
      </div>
    )
  }

  const SortHeader = ({
    field,
    label,
    className = '',
  }: {
    field: SortField
    label: string
    className?: string
  }) => (
    <th
      className={`px-3 py-2.5 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider cursor-pointer hover:text-gray-700 dark:hover:text-gray-300 select-none ${className}`}
      onClick={() => handleSort(field)}
    >
      <span className="inline-flex items-center gap-1">
        {label}
        {sortField === field && (
          <span className="text-blue-500">
            {sortDir === 'asc' ? '\u2191' : '\u2193'}
          </span>
        )}
      </span>
    </th>
  )

  return (
    <div className="overflow-x-auto" role="table" aria-label="Category breakdown table">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-gray-200 dark:border-gray-700">
            <SortHeader field="name" label="Category" className="min-w-[200px]" />
            <SortHeader field="count" label="Sessions" />
            <SortHeader field="percentage" label="%" />
            <SortHeader field="avgReeditRate" label="Re-edit" />
            <SortHeader field="avgDuration" label="Avg Dur" />
            <SortHeader field="commitRate" label="Commits" />
            <th className="px-3 py-2.5 w-8"></th>
          </tr>
        </thead>
        <tbody>
          {sortedData.map((l1) => (
            <CategoryRows
              key={l1.id}
              node={l1}
              depth={0}
              expandedIds={expandedIds}
              selectedCategory={selectedCategory}
              onToggle={toggleExpand}
              onCategoryClick={onCategoryClick}
            />
          ))}
        </tbody>
      </table>
    </div>
  )
}

function CategoryRows({
  node,
  depth,
  expandedIds,
  selectedCategory,
  onToggle,
  onCategoryClick,
}: {
  node: CategoryNode
  depth: number
  expandedIds: Set<string>
  selectedCategory: string | null
  onToggle: (id: string) => void
  onCategoryClick: (id: string) => void
}) {
  const isExpanded = expandedIds.has(node.id)
  const hasChildren = (node.children?.length ?? 0) > 0
  const isSelected = selectedCategory === node.id

  return (
    <>
      <tr
        className={`
          border-b border-gray-100 dark:border-gray-800
          hover:bg-gray-50 dark:hover:bg-gray-800/50
          ${isSelected ? 'bg-blue-50 dark:bg-blue-900/20' : ''}
          cursor-pointer transition-colors
        `}
        onClick={() => onCategoryClick(node.id)}
      >
        <td className="px-3 py-2.5">
          <div
            className="flex items-center gap-1"
            style={{ paddingLeft: `${depth * 20}px` }}
          >
            {hasChildren ? (
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  onToggle(node.id)
                }}
                className="p-0.5 hover:bg-gray-200 dark:hover:bg-gray-700 rounded"
                aria-label={isExpanded ? 'Collapse' : 'Expand'}
              >
                {isExpanded ? (
                  <ChevronDown className="w-4 h-4 text-gray-500" />
                ) : (
                  <ChevronRight className="w-4 h-4 text-gray-500" />
                )}
              </button>
            ) : (
              <span className="w-5" />
            )}
            <span className="font-medium text-gray-900 dark:text-gray-100">
              {node.name}
            </span>
          </div>
        </td>
        <td className="px-3 py-2.5 text-gray-700 dark:text-gray-300 tabular-nums">
          {node.count}
        </td>
        <td className="px-3 py-2.5 text-gray-700 dark:text-gray-300 tabular-nums">
          {node.percentage.toFixed(1)}%
        </td>
        <td className="px-3 py-2.5 text-gray-700 dark:text-gray-300 tabular-nums">
          {(node.avgReeditRate * 100).toFixed(0)}%
        </td>
        <td className="px-3 py-2.5 text-gray-700 dark:text-gray-300 tabular-nums">
          {formatDuration(node.avgDuration)}
        </td>
        <td className="px-3 py-2.5 text-gray-700 dark:text-gray-300 tabular-nums">
          {node.commitRate.toFixed(0)}%
        </td>
        <td className="px-3 py-2.5">
          {hasChildren && (
            <ChevronRight className="w-4 h-4 text-gray-400" />
          )}
        </td>
      </tr>
      {isExpanded &&
        node.children?.map((child) => (
          <CategoryRows
            key={child.id}
            node={child}
            depth={depth + 1}
            expandedIds={expandedIds}
            selectedCategory={selectedCategory}
            onToggle={onToggle}
            onCategoryClick={onCategoryClick}
          />
        ))}
    </>
  )
}
