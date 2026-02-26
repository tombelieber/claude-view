import { useState, useMemo, useCallback } from 'react'
import { Treemap, ResponsiveContainer, Tooltip } from 'recharts'
import type { CategoryNode } from '../../types/generated/CategoryNode'

// Color mapping for L1 categories
const CATEGORY_COLORS: Record<string, { fill: string; hover: string }> = {
  code_work: { fill: '#3B82F6', hover: '#2563EB' },
  support_work: { fill: '#10B981', hover: '#059669' },
  thinking_work: { fill: '#8B5CF6', hover: '#7C3AED' },
  uncategorized: { fill: '#6B7280', hover: '#4B5563' },
}

interface TreemapProps {
  data: CategoryNode[]
  onCategoryClick: (categoryId: string) => void
  selectedCategory: string | null
}

// Get root L1 category id from any hierarchical id
function getRootId(id: string): string {
  return id.split('/')[0]
}

export function CategoryTreemap({
  data,
  onCategoryClick,
  selectedCategory,
}: TreemapProps) {
  const [hoveredId, setHoveredId] = useState<string | null>(null)

  // Transform data for Recharts treemap format
  const treemapData = useMemo(() => {
    return data.map((l1) => ({
      name: l1.name,
      id: l1.id,
      size: l1.count,
      percentage: l1.percentage,
      fill: CATEGORY_COLORS[l1.id]?.fill ?? '#6B7280',
      children: l1.children?.map((l2) => ({
        name: l2.name,
        id: l2.id,
        size: l2.count,
        percentage: l2.percentage,
        parentId: l1.id,
        fill: CATEGORY_COLORS[l1.id]?.fill ?? '#6B7280',
      })) ?? [],
    }))
  }, [data])

  const handleClick = useCallback(
    (nodeId: string) => {
      onCategoryClick(nodeId)
    },
    [onCategoryClick],
  )

  // Custom treemap cell renderer
  const CustomContent = useCallback(
    (props: Record<string, unknown>) => {
      const x = props.x as number
      const y = props.y as number
      const width = props.width as number
      const height = props.height as number
      const name = props.name as string
      const percentage = props.percentage as number | undefined
      const id = (props.id ?? props.parentId) as string | undefined

      if (width < 40 || height < 25 || !id) return null

      const rootId = getRootId(id)
      const isHovered = hoveredId === id
      const isSelected = selectedCategory === id

      return (
        <g>
          <rect
            x={x}
            y={y}
            width={width}
            height={height}
            style={{
              fill:
                CATEGORY_COLORS[rootId]?.fill ?? '#6B7280',
              stroke: isSelected
                ? '#FFF'
                : isHovered
                  ? '#FFF'
                  : '#1F2937',
              strokeWidth: isSelected ? 3 : isHovered ? 2 : 1,
              opacity: isHovered || isSelected ? 1 : 0.85,
              cursor: 'pointer',
              transition: 'all 150ms ease-out',
            }}
            onClick={() => handleClick(id)}
            onMouseEnter={() => setHoveredId(id)}
            onMouseLeave={() => setHoveredId(null)}
          />
          {width > 50 && height > 30 && (
            <>
              <text
                x={x + width / 2}
                y={y + height / 2 - (height > 50 ? 8 : 0)}
                textAnchor="middle"
                fill="#FFF"
                fontSize={width > 100 ? 13 : 11}
                fontWeight={600}
                style={{ pointerEvents: 'none' }}
              >
                {name}
              </text>
              {height > 50 && percentage !== undefined && (
                <text
                  x={x + width / 2}
                  y={y + height / 2 + 12}
                  textAnchor="middle"
                  fill="#FFF"
                  fontSize={11}
                  opacity={0.8}
                  style={{ pointerEvents: 'none' }}
                >
                  {percentage.toFixed(0)}%
                </text>
              )}
            </>
          )}
        </g>
      )
    },
    [hoveredId, selectedCategory, handleClick],
  )

  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center h-[400px] text-gray-400 dark:text-gray-500">
        No category data available
      </div>
    )
  }

  return (
    <div className="w-full h-[400px]" role="img" aria-label="Category treemap visualization">
      <ResponsiveContainer width="100%" height="100%">
        <Treemap
          data={treemapData}
          dataKey="size"
          aspectRatio={4 / 3}
          stroke="#1F2937"
          content={<CustomContent />}
        >
          <Tooltip
            content={({ payload }) => {
              if (!payload?.[0]) return null
              const d = payload[0].payload as Record<string, unknown>
              return (
                <div className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm">
                  <div className="font-semibold">{d.name as string}</div>
                  <div className="text-gray-300">
                    {d.size as number} sessions ({(d.percentage as number).toFixed(1)}%)
                  </div>
                  <div className="text-gray-400 text-xs mt-1">
                    Click to drill down
                  </div>
                </div>
              )
            }}
          />
        </Treemap>
      </ResponsiveContainer>
    </div>
  )
}
