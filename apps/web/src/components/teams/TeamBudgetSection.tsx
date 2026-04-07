import { ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { formatCostUsd, formatTokenCount } from '../../lib/format-utils'
import { formatModelName } from '../../lib/format-model'
import { cn } from '../../lib/utils'
import type { TeamCostBreakdown } from '../../types/generated/TeamCostBreakdown'
import type { TeamMemberCost } from '../../types/generated/TeamMemberCost'

const COLOR_MAP: Record<string, string> = {
  blue: 'bg-blue-500',
  green: 'bg-green-500',
  yellow: 'bg-yellow-500',
  purple: 'bg-purple-500',
  red: 'bg-red-500',
  orange: 'bg-orange-500',
}

interface TeamBudgetSectionProps {
  cost: TeamCostBreakdown
}

export function TeamBudgetSection({ cost }: TeamBudgetSectionProps) {
  const [expanded, setExpanded] = useState(false)
  const membersWithCost = cost.members.filter((m) => m.costUsd != null && m.costUsd > 0)
  const totalMemberCost = membersWithCost.reduce((sum, m) => sum + (m.costUsd ?? 0), 0)
  const grandTotal = cost.leadCostUsd + totalMemberCost
  const allInProcess = cost.members.length > 0 && cost.members.every((m) => m.inProcess)

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3 space-y-3">
      {/* Header: label + total */}
      <div className="flex items-baseline justify-between">
        <span className="text-xs font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
          Team Cost
        </span>
        <span className="text-sm font-mono font-semibold text-gray-900 dark:text-gray-100 tabular-nums">
          {formatCostUsd(grandTotal)}
        </span>
      </div>

      {/* Proportion bar */}
      {grandTotal > 0 && (
        <div className="flex h-2 rounded-full overflow-hidden bg-gray-200 dark:bg-gray-800">
          {/* Lead portion */}
          {cost.leadCostUsd > 0 && (
            <div
              className="bg-gray-400 dark:bg-gray-500 transition-all"
              style={{ width: `${(cost.leadCostUsd / grandTotal) * 100}%` }}
              title={`Lead session: ${formatCostUsd(cost.leadCostUsd)}`}
            />
          )}
          {/* Member portions */}
          {membersWithCost.map((m) => (
            <div
              key={m.name}
              className={cn(COLOR_MAP[m.color] || 'bg-gray-400', 'transition-all')}
              style={{ width: `${((m.costUsd ?? 0) / grandTotal) * 100}%` }}
              title={`${m.name}: ${formatCostUsd(m.costUsd ?? 0)}`}
            />
          ))}
        </div>
      )}

      {/* Per-member rows */}
      <div className="space-y-1.5">
        {/* Lead session */}
        <MemberCostRow
          name="Lead session"
          color="gray"
          model=""
          costUsd={cost.leadCostUsd}
          pct={grandTotal > 0 ? (cost.leadCostUsd / grandTotal) * 100 : 0}
        />
        {/* Members */}
        {cost.members.map((m) => (
          <MemberCostRow
            key={m.name}
            name={m.name}
            color={m.color}
            model={m.model}
            costUsd={m.costUsd}
            inProcess={m.inProcess}
            pct={grandTotal > 0 ? ((m.costUsd ?? 0) / grandTotal) * 100 : 0}
          />
        ))}
      </div>

      {/* Limitation note for in-process teams */}
      {allInProcess && (
        <p className="text-[10px] text-gray-400 dark:text-gray-500 leading-snug">
          Per-member cost breakdown unavailable — in-process teammates share the lead session.
        </p>
      )}

      {/* Expandable token details */}
      {membersWithCost.length > 0 && (
        <div>
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="flex items-center gap-1 text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          >
            {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            Token details
          </button>
          {expanded && (
            <div className="mt-2 space-y-2 text-xs">
              {membersWithCost.map((m) => (
                <MemberTokenDetail key={m.name} member={m} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function MemberCostRow({
  name,
  color,
  model,
  costUsd,
  inProcess,
  pct,
}: {
  name: string
  color: string
  model: string
  costUsd: number | null
  inProcess?: boolean
  pct: number
}) {
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className={cn('w-2 h-2 rounded-full shrink-0', COLOR_MAP[color] || 'bg-gray-400')} />
      <span className="text-gray-700 dark:text-gray-300 truncate">{name}</span>
      {model && (
        <span className="text-xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 shrink-0">
          {formatModelName(model)}
        </span>
      )}
      <span className="flex-1" />
      {inProcess ? (
        <span className="text-xs text-gray-400 dark:text-gray-500 italic shrink-0">
          incl. in coordinator
        </span>
      ) : (
        <>
          <span className="font-mono tabular-nums text-gray-700 dark:text-gray-300 shrink-0">
            {costUsd != null ? formatCostUsd(costUsd) : '—'}
          </span>
          <span className="font-mono tabular-nums text-gray-400 dark:text-gray-500 w-10 text-right shrink-0">
            {costUsd != null ? `${pct.toFixed(0)}%` : ''}
          </span>
        </>
      )}
    </div>
  )
}

function MemberTokenDetail({ member }: { member: TeamMemberCost }) {
  if (!member.tokens) return null
  const t = member.tokens
  return (
    <div className="ml-4 border-l-2 border-gray-200 dark:border-gray-700 pl-2">
      <span className="font-medium text-gray-600 dark:text-gray-400">{member.name}</span>
      <div className="font-mono tabular-nums text-gray-400 dark:text-gray-500 mt-0.5">
        {[
          t.inputTokens ? `${formatTokenCount(t.inputTokens)} in` : null,
          t.outputTokens ? `${formatTokenCount(t.outputTokens)} out` : null,
          t.cacheReadTokens ? `${formatTokenCount(t.cacheReadTokens)} cache` : null,
        ]
          .filter(Boolean)
          .join(' · ')}
      </div>
    </div>
  )
}
