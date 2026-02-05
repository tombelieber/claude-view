import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts'
import { Link } from 'react-router-dom'
import { Layers } from 'lucide-react'
import type { CategoryDataPoint } from '../../types/generated/CategoryDataPoint'

// ============================================================================
// Types
// ============================================================================

interface CategoryEvolutionChartProps {
  data: CategoryDataPoint[] | null
  insight: string | null
  classificationRequired: boolean
}

// ============================================================================
// Constants
// ============================================================================

const CATEGORY_COLORS = {
  codeWork: '#3B82F6',
  supportWork: '#F59E0B',
  thinkingWork: '#8B5CF6',
} as const

const CATEGORY_LABELS: Record<string, string> = {
  codeWork: 'Code Work',
  supportWork: 'Support Work',
  thinkingWork: 'Thinking Work',
}

// ============================================================================
// Helpers
// ============================================================================

function formatDateLabel(date: string): string {
  if (date.includes('W')) {
    const parts = date.split('-W')
    return `W${parts[1]}`
  }
  try {
    return new Date(date + 'T00:00:00').toLocaleDateString('en-US', {
      month: 'short',
    })
  } catch {
    return date
  }
}

function formatDateTooltip(date: string): string {
  if (date.includes('W')) {
    const parts = date.split('-W')
    return `${parts[0]} Week ${parts[1]}`
  }
  try {
    return new Date(date + 'T00:00:00').toLocaleDateString('en-US', {
      month: 'long',
      year: 'numeric',
    })
  } catch {
    return date
  }
}

// ============================================================================
// Component
// ============================================================================

export function CategoryEvolutionChart({
  data,
  insight,
  classificationRequired,
}: CategoryEvolutionChartProps) {
  // Classification required state
  if (classificationRequired || !data) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4 flex items-center gap-2">
          <Layers className="h-5 w-5 text-gray-400" />
          Category Evolution
        </h3>
        <div className="flex flex-col items-center justify-center py-12 px-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
          <div className="text-3xl mb-3">&#x1F4CA;</div>
          <h4 className="text-base font-medium text-gray-900 dark:text-gray-100 mb-2">
            Classification Required
          </h4>
          <p className="text-sm text-gray-600 dark:text-gray-400 text-center mb-4 max-w-sm">
            Category breakdown requires session classification. Go to System to
            classify your sessions and enable this chart.
          </p>
          <Link
            to="/system?tab=classification"
            className="px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
          >
            Classify Sessions
          </Link>
        </div>
      </div>
    )
  }

  if (data.length === 0) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4 flex items-center gap-2">
          <Layers className="h-5 w-5 text-gray-400" />
          Category Evolution
        </h3>
        <div className="flex items-center justify-center py-16 text-gray-500 dark:text-gray-400 text-sm">
          No category data for this period.
        </div>
      </div>
    )
  }

  // Calculate latest percentages for legend
  const latest = data[data.length - 1]

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4 flex items-center gap-2">
        <Layers className="h-5 w-5 text-gray-400" />
        Category Evolution
      </h3>

      <ResponsiveContainer width="100%" height={300}>
        <AreaChart
          data={data}
          margin={{ top: 10, right: 30, left: 0, bottom: 0 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="#374151" opacity={0.2} />
          <XAxis
            dataKey="date"
            tickFormatter={formatDateLabel}
            stroke="#9CA3AF"
            fontSize={12}
            tickLine={false}
          />
          <YAxis
            tickFormatter={(value) => `${(value * 100).toFixed(0)}%`}
            stroke="#9CA3AF"
            fontSize={12}
            tickLine={false}
            domain={[0, 1]}
          />
          <Tooltip
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            formatter={((value: number, name: string) => [
              `${(value * 100).toFixed(1)}%`,
              CATEGORY_LABELS[name] || name,
            ]) as any}
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            labelFormatter={formatDateTooltip as any}
            contentStyle={{
              backgroundColor: '#1F2937',
              border: 'none',
              borderRadius: '8px',
              color: '#F9FAFB',
              fontSize: '13px',
            }}
            itemStyle={{ color: '#F9FAFB' }}
            labelStyle={{ color: '#9CA3AF', marginBottom: '4px' }}
          />
          <Area
            type="monotone"
            dataKey="thinkingWork"
            stackId="1"
            stroke={CATEGORY_COLORS.thinkingWork}
            fill={CATEGORY_COLORS.thinkingWork}
            fillOpacity={0.8}
          />
          <Area
            type="monotone"
            dataKey="supportWork"
            stackId="1"
            stroke={CATEGORY_COLORS.supportWork}
            fill={CATEGORY_COLORS.supportWork}
            fillOpacity={0.8}
          />
          <Area
            type="monotone"
            dataKey="codeWork"
            stackId="1"
            stroke={CATEGORY_COLORS.codeWork}
            fill={CATEGORY_COLORS.codeWork}
            fillOpacity={0.8}
          />
        </AreaChart>
      </ResponsiveContainer>

      {/* Legend */}
      <div className="flex flex-wrap items-center gap-4 mt-3 text-sm text-gray-600 dark:text-gray-400">
        <div className="flex items-center gap-1.5">
          <div
            className="w-3 h-3 rounded-sm"
            style={{ backgroundColor: CATEGORY_COLORS.codeWork }}
          />
          <span>Code Work ({(latest.codeWork * 100).toFixed(0)}%)</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div
            className="w-3 h-3 rounded-sm"
            style={{ backgroundColor: CATEGORY_COLORS.supportWork }}
          />
          <span>Support Work ({(latest.supportWork * 100).toFixed(0)}%)</span>
        </div>
        <div className="flex items-center gap-1.5">
          <div
            className="w-3 h-3 rounded-sm"
            style={{ backgroundColor: CATEGORY_COLORS.thinkingWork }}
          />
          <span>
            Thinking Work ({(latest.thinkingWork * 100).toFixed(0)}%)
          </span>
        </div>
      </div>

      {insight && (
        <p className="mt-4 text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
          <span className="shrink-0 mt-0.5 text-amber-500">*</span>
          <span>{insight}</span>
        </p>
      )}
    </div>
  )
}
