import { useMemo } from 'react'
import { Cpu, Check, ChevronRight } from 'lucide-react'
import { formatNumber, formatCostUsd } from '../../lib/format-utils'
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip as RechartsTooltip,
  ResponsiveContainer,
  Cell,
  LabelList,
} from 'recharts'
import type { ModelStats } from '../../types/generated'
import { MetricTooltip } from './MetricTooltip'

interface ModelComparisonProps {
  byModel: ModelStats[]
}

/**
 * ModelComparison displays a horizontal bar chart of lines per model
 * with a collapsible detailed table comparing model performance.
 *
 * Shows lines produced, re-edit rate, cost per line, and best use case per model.
 */
export function ModelComparison({ byModel }: ModelComparisonProps) {
  // Sort models by lines descending for chart
  const chartData = useMemo(
    () =>
      [...byModel]
        .sort((a, b) => b.lines - a.lines)
        .map((m) => ({
          displayName: formatModelFamily(m.model),
          lines: m.lines,
          model: m.model,
          reeditRate: m.reeditRate,
          costPerLine: m.costPerLine,
        })),
    [byModel]
  )

  if (byModel.length === 0) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center gap-2 mb-4">
          <Cpu className="w-4 h-4 text-indigo-500" aria-hidden="true" />
          <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            By Model
          </h2>
        </div>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          No model data available for this period.
        </p>
      </div>
    )
  }

  // Find the best (lowest) re-edit rate
  const bestReeditRate = Math.min(
    ...byModel.filter((m) => m.reeditRate !== null).map((m) => m.reeditRate!)
  )

  // Generate summary insight
  const summaryInsight = generateModelInsight(byModel)

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex items-center gap-2 mb-4">
        <Cpu className="w-4 h-4 text-indigo-500" aria-hidden="true" />
        <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          By Model
        </h2>
      </div>

      {/* Horizontal Bar Chart */}
      <div style={{ height: `${chartData.length * 48 + 32}px` }} className="mb-4 overflow-hidden">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart
            data={chartData}
            layout="vertical"
            margin={{ top: 5, right: 60, left: 0, bottom: 5 }}
          >
            <XAxis
              type="number"
              tick={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }}
              tickFormatter={(v: number) => formatNumber(v)}
            />
            <YAxis
              type="category"
              dataKey="displayName"
              tick={{ fontSize: 12, fill: 'var(--chart-text, #6b7280)' }}
              width={70}
            />
            <RechartsTooltip
              contentStyle={{
                backgroundColor: 'var(--tooltip-bg, #fff)',
                border: '1px solid var(--tooltip-border, #e5e7eb)',
                borderRadius: '8px',
                fontSize: '12px',
              }}
              formatter={(value: number, _name: string, props: any) => {
                const item = props.payload
                const parts = [`${formatNumber(value)} lines`]
                if (item.reeditRate !== null)
                  parts.push(`Re-edit: ${item.reeditRate.toFixed(2)}`)
                if (item.costPerLine !== null)
                  parts.push(`Cost/line: ${formatCostUsd(item.costPerLine)}`)
                return [parts.join(' \u00b7 '), item.displayName]
              }}
            />
            <Bar dataKey="lines" radius={[0, 4, 4, 0]}>
              {chartData.map((entry) => (
                <Cell key={entry.model} fill={getModelColor(entry.model)} />
              ))}
              <LabelList
                dataKey="lines"
                position="right"
                formatter={(v: number) => formatNumber(v)}
                style={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }}
              />
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>

      {/* Collapsible Detailed Table */}
      <details className="mt-4 group">
        <summary className="text-sm text-gray-500 dark:text-gray-400 cursor-pointer hover:text-gray-700 dark:hover:text-gray-300 motion-safe:transition-colors motion-safe:duration-200 list-none flex items-center gap-1">
          <ChevronRight className="w-4 h-4 motion-safe:transition-transform motion-safe:duration-200 group-open:rotate-90" />
          Show detailed table
        </summary>
        <div className="mt-3 overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-200 dark:border-gray-700">
                <th className="text-left py-2 pr-4 font-medium text-gray-500 dark:text-gray-400">
                  Model
                </th>
                <th className="text-right py-2 px-4 font-medium text-gray-500 dark:text-gray-400">
                  Lines
                </th>
                <th className="text-right py-2 px-4 font-medium text-gray-500 dark:text-gray-400">
                  <span className="inline-flex items-center">
                    Re-edit Rate
                    <MetricTooltip>
                      <span className="font-medium text-gray-900 dark:text-gray-100">Re-edit rate</span> measures how often AI-generated files need further editing after the initial write.
                      <br /><br />
                      <span className="font-medium text-gray-900 dark:text-gray-100">Lower is better.</span> 0 = no re-edits needed.
                      <br /><br />
                      Formula: files re-edited / total files edited
                    </MetricTooltip>
                  </span>
                </th>
                <th className="text-right py-2 px-4 font-medium text-gray-500 dark:text-gray-400">
                  Cost/Line
                </th>
                <th className="text-left py-2 pl-4 font-medium text-gray-500 dark:text-gray-400">
                  Best For
                </th>
              </tr>
            </thead>
            <tbody>
              {byModel.map((model) => {
                const isBestReedit =
                  model.reeditRate !== null && model.reeditRate === bestReeditRate

                return (
                  <tr
                    key={model.model}
                    className="border-b border-gray-100 dark:border-gray-800 last:border-0"
                  >
                    <td className="py-3 pr-4">
                      <span className="font-medium text-gray-900 dark:text-gray-100">
                        {formatModelFamily(model.model)}
                      </span>
                    </td>
                    <td className="py-3 px-4 text-right tabular-nums text-gray-700 dark:text-gray-300">
                      {formatNumber(model.lines)}
                    </td>
                    <td className="py-3 px-4 text-right tabular-nums">
                      <span className="inline-flex items-center gap-1">
                        <span className="text-gray-700 dark:text-gray-300">
                          {model.reeditRate !== null ? model.reeditRate.toFixed(2) : '--'}
                        </span>
                        {isBestReedit && (
                          <Check
                            className="w-4 h-4 text-green-500"
                            aria-label="Best re-edit rate"
                          />
                        )}
                      </span>
                    </td>
                    <td className="py-3 px-4 text-right tabular-nums text-gray-700 dark:text-gray-300">
                      {model.costPerLine !== null ? formatCostUsd(model.costPerLine) : '--'}
                    </td>
                    <td className="py-3 pl-4 text-gray-600 dark:text-gray-400">
                      {model.insight || getDefaultInsight(model.model)}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      </details>

      {/* Summary Insight */}
      {summaryInsight && (
        <p className="mt-4 text-sm text-gray-600 dark:text-gray-400 leading-relaxed">
          {summaryInsight}
        </p>
      )}
    </div>
  )
}

/**
 * Get color for model bar in chart.
 */
function getModelColor(model: string): string {
  const lower = model.toLowerCase()
  if (lower.includes('opus')) return '#3b82f6'
  if (lower.includes('sonnet')) return '#8b5cf6'
  if (lower.includes('haiku')) return '#f59e0b'
  return '#6b7280'
}

/**
 * Extract model family name for space-constrained chart labels (e.g. "Opus", "Sonnet").
 */
function formatModelFamily(model: string): string {
  // Handle common Claude model naming patterns
  if (model.toLowerCase().includes('opus')) return 'Opus'
  if (model.toLowerCase().includes('sonnet')) return 'Sonnet'
  if (model.toLowerCase().includes('haiku')) return 'Haiku'
  // Fallback: capitalize first letter
  return model.charAt(0).toUpperCase() + model.slice(1)
}

/**
 * Get default insight for model type.
 */
function getDefaultInsight(model: string): string {
  const lower = model.toLowerCase()
  if (lower.includes('opus')) return 'Complex features'
  if (lower.includes('sonnet')) return 'Standard work'
  if (lower.includes('haiku')) return 'Quick questions'
  return ''
}

/**
 * Generate summary insight comparing models.
 */
function generateModelInsight(models: ModelStats[]): string | null {
  if (models.length < 2) return null

  // Find the model with lowest re-edit rate and highest cost
  const withReedit = models.filter((m) => m.reeditRate !== null && m.costPerLine !== null)
  if (withReedit.length < 2) return null

  const sorted = [...withReedit].sort((a, b) => (a.reeditRate ?? 0) - (b.reeditRate ?? 0))
  const best = sorted[0]
  const worst = sorted[sorted.length - 1]

  if (best.model === worst.model) return null

  const costRatio = (best.costPerLine ?? 0) / (worst.costPerLine ?? 1)
  const reeditReduction = (1 - (best.reeditRate ?? 0) / (worst.reeditRate ?? 1)) * 100

  if (costRatio > 1 && reeditReduction > 10) {
    return `${formatModelFamily(best.model)} costs ${costRatio.toFixed(1)}x more but needs ${reeditReduction.toFixed(0)}% fewer re-edits \u2014 worth it for complex work; use ${formatModelFamily(worst.model)} for routine tasks to save cost.`
  }

  return null
}

/**
 * Format large numbers with K/M suffixes.
 */
