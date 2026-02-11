import { useMemo, useCallback } from 'react'
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from 'recharts'
import type { CategoryNode } from '../../types/generated/CategoryNode'

// Color mapping for L1 categories
const CATEGORY_COLORS: Record<string, string> = {
  code_work: '#3B82F6',
  support_work: '#10B981',
  thinking_work: '#8B5CF6',
}

interface BarChartProps {
  data: CategoryNode[]
  onCategoryClick: (categoryId: string) => void
  selectedCategory: string | null
}

interface FlatCategory {
  name: string
  id: string
  rootId: string
  count: number
  percentage: number
}

export function CategoryBarChart({
  data,
  onCategoryClick,
  selectedCategory,
}: BarChartProps) {
  // Flatten L1 -> L2 for bar chart
  const flatData = useMemo((): FlatCategory[] => {
    const result: FlatCategory[] = []
    for (const l1 of data) {
      if (l1.children && l1.children.length > 0) {
        for (const l2 of l1.children) {
          result.push({
            name: `${l1.name} > ${l2.name}`,
            id: l2.id,
            rootId: l1.id,
            count: l2.count,
            percentage: l2.percentage,
          })
        }
      } else {
        result.push({
          name: l1.name,
          id: l1.id,
          rootId: l1.id,
          count: l1.count,
          percentage: l1.percentage,
        })
      }
    }
    return result.sort((a, b) => b.count - a.count)
  }, [data])

  const handleBarClick = useCallback(
    (entry: FlatCategory) => {
      onCategoryClick(entry.id)
    },
    [onCategoryClick],
  )

  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center h-[400px] text-gray-400 dark:text-gray-500">
        No category data available
      </div>
    )
  }

  return (
    <div className="w-full h-[400px]" role="img" aria-label="Category bar chart">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={flatData} layout="vertical" margin={{ left: 20 }}>
          <XAxis type="number" hide />
          <YAxis
            type="category"
            dataKey="name"
            width={160}
            tick={{ fontSize: 12 }}
          />
          <Tooltip
            content={({ payload }) => {
              if (!payload?.[0]) return null
              const d = payload[0].payload as FlatCategory
              return (
                <div className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm">
                  <div className="font-semibold">{d.name}</div>
                  <div className="text-gray-300">
                    {d.count} sessions ({d.percentage.toFixed(1)}%)
                  </div>
                </div>
              )
            }}
          />
          <Bar
            dataKey="count"
            radius={[0, 4, 4, 0]}
            onClick={(_, index) => {
              const entry = flatData[index]
              if (entry) handleBarClick(entry)
            }}
            style={{ cursor: 'pointer' }}
          >
            {flatData.map((entry) => (
              <Cell
                key={entry.id}
                fill={CATEGORY_COLORS[entry.rootId] ?? '#6B7280'}
                opacity={selectedCategory === entry.id ? 1 : 0.8}
              />
            ))}
          </Bar>
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}
