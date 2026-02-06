import { Cpu, Check } from 'lucide-react'
import type { ModelStats } from '../../types/generated'

interface ModelComparisonProps {
  byModel: ModelStats[]
}

/**
 * ModelComparison displays a table comparing model performance.
 *
 * Shows lines produced, re-edit rate, cost per line, and best use case per model.
 */
export function ModelComparison({ byModel }: ModelComparisonProps) {
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

      {/* Table */}
      <div className="overflow-x-auto">
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
                Re-edit Rate
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
              const isBestReedit = model.reeditRate !== null && model.reeditRate === bestReeditRate

              return (
                <tr
                  key={model.model}
                  className="border-b border-gray-100 dark:border-gray-800 last:border-0"
                >
                  <td className="py-3 pr-4">
                    <span className="font-medium text-gray-900 dark:text-gray-100">
                      {formatModelName(model.model)}
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
                    {model.costPerLine !== null ? `$${model.costPerLine.toFixed(4)}` : '--'}
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
 * Format model name for display (capitalize, handle claude-X-Y-sonnet format).
 */
function formatModelName(model: string): string {
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
    return `${formatModelName(best.model)} costs ${costRatio.toFixed(1)}x more but needs ${reeditReduction.toFixed(0)}% fewer re-edits \u2014 worth it for complex work; use ${formatModelName(worst.model)} for routine tasks to save cost.`
  }

  return null
}

/**
 * Format large numbers with K/M suffixes.
 */
function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return n.toLocaleString()
}
