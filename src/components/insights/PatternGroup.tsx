import { ArrowUp, ArrowRight, Eye } from 'lucide-react'
import { PatternCard } from './PatternCard'
import { cn } from '../../lib/utils'
import type { GeneratedInsight } from '../../types/generated/GeneratedInsight'

interface PatternGroupProps {
  tier: 'high' | 'medium' | 'observations'
  patterns: GeneratedInsight[]
}

const TIER_CONFIG = {
  high: {
    label: 'HIGH IMPACT',
    icon: ArrowUp,
    color: 'text-blue-700 dark:text-blue-400',
    iconColor: 'text-blue-600 dark:text-blue-400',
  },
  medium: {
    label: 'MEDIUM IMPACT',
    icon: ArrowRight,
    color: 'text-gray-600 dark:text-gray-400',
    iconColor: 'text-gray-500 dark:text-gray-400',
  },
  observations: {
    label: 'OBSERVATIONS',
    icon: Eye,
    color: 'text-gray-500 dark:text-gray-500',
    iconColor: 'text-gray-400 dark:text-gray-500',
  },
} as const

export function PatternGroup({ tier, patterns }: PatternGroupProps) {
  if (patterns.length === 0) return null

  const config = TIER_CONFIG[tier]
  const Icon = config.icon

  return (
    <div>
      <div className="flex items-center gap-2 mb-3">
        <Icon className={cn('w-4 h-4', config.iconColor)} />
        <h3 className={cn('text-xs font-semibold uppercase tracking-wider', config.color)}>
          {config.label} ({patterns.length})
        </h3>
      </div>
      <div className="space-y-3">
        {patterns.map((pattern) => (
          <PatternCard key={pattern.patternId} pattern={pattern} />
        ))}
      </div>
    </div>
  )
}
