import { ArrowDown, ArrowUp, FolderOpen, GitBranch } from 'lucide-react'
import { useMemo, useState } from 'react'
import { formatCostUsd } from '../../lib/format-utils'
import { cn } from '../../lib/utils'
import { cleanPreviewText } from '../../utils/get-session-title'
import { ContextBar } from './ContextBar'
import { StateBadge } from './SessionCard'
import { StatusDot } from './StatusDot'
import { hasUnavailableCost } from './cost-display'
import { GROUP_ORDER } from './types'
import { type LiveSession, sessionTotalCost } from './use-live-sessions'

interface ListViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
}

type SortColumn = 'status' | 'project' | 'branch' | 'turns' | 'cost' | 'context' | 'lastActive'
type SortDir = 'asc' | 'desc'

function getContextPercent(session: LiveSession): number | null {
  if (session.statuslineUsedPct != null) return Math.min(100, Math.round(session.statuslineUsedPct))
  return null
}

function formatRelativeTime(ts: number): string {
  const diff = Date.now() / 1000 - ts
  if (diff < 60) return `${Math.floor(diff)}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

function formatCost(session: LiveSession): string {
  const usd = sessionTotalCost(session)
  return hasUnavailableCost(usd, session.cost, session.tokens.totalTokens)
    ? 'Unavailable'
    : formatCostUsd(usd)
}

const COLUMNS: { key: SortColumn | 'activity'; label: string; width: string; sortable: boolean }[] =
  [
    { key: 'status', label: 'Status', width: 'w-[40px]', sortable: true },
    { key: 'project', label: 'Project', width: 'w-[140px]', sortable: true },
    { key: 'branch', label: 'Branch', width: 'w-[120px]', sortable: true },
    { key: 'activity', label: 'Activity', width: 'flex-1', sortable: false },
    { key: 'turns', label: 'Turns', width: 'w-[60px]', sortable: true },
    { key: 'cost', label: 'Cost', width: 'w-[70px]', sortable: true },
    { key: 'context', label: 'Context%', width: 'w-[65px]', sortable: true },
    { key: 'lastActive', label: 'Last Active', width: 'w-[90px]', sortable: true },
  ]

export function ListView({ sessions, selectedId, onSelect }: ListViewProps) {
  const [sortColumn, setSortColumn] = useState<SortColumn>('status')
  const [sortDir, setSortDir] = useState<SortDir>('asc')

  const sorted = useMemo(() => {
    const arr = [...sessions]
    arr.sort((a, b) => {
      let cmp = 0
      switch (sortColumn) {
        case 'status': {
          const aOrder = GROUP_ORDER[a.agentState.group]
          const bOrder = GROUP_ORDER[b.agentState.group]
          cmp = aOrder - bOrder
          break
        }
        case 'project':
          cmp = (a.projectDisplayName || a.project).localeCompare(b.projectDisplayName || b.project)
          break
        case 'branch':
          cmp = (a.effectiveBranch ?? '').localeCompare(b.effectiveBranch ?? '')
          break
        case 'turns':
          cmp = a.turnCount - b.turnCount
          break
        case 'cost':
          cmp = sessionTotalCost(a) - sessionTotalCost(b)
          break
        case 'context': {
          const aPct = getContextPercent(a) ?? -1
          const bPct = getContextPercent(b) ?? -1
          cmp = aPct - bPct
          break
        }
        case 'lastActive':
          cmp = a.lastActivityAt - b.lastActivityAt
          break
      }
      if (cmp === 0) cmp = b.lastActivityAt - a.lastActivityAt
      return sortDir === 'desc' ? -cmp : cmp
    })
    return arr
  }, [sessions, sortColumn, sortDir])

  function handleHeaderClick(col: SortColumn) {
    if (sortColumn === col) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortColumn(col)
      setSortDir('asc')
    }
  }

  if (sessions.length === 0) {
    return null
  }

  return (
    <div className="w-full overflow-x-auto">
      <table className="w-full table-fixed border-collapse bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-800">
        <thead className="sticky top-0 bg-gray-100/90 dark:bg-gray-800/90 backdrop-blur-sm z-10">
          <tr>
            {COLUMNS.map((col) => (
              <th
                key={col.key}
                className={cn(
                  'px-2 py-2 text-left text-[10px] uppercase tracking-wider font-semibold text-gray-400 dark:text-gray-500',
                  col.width === 'flex-1' ? '' : col.width,
                  col.sortable &&
                    'cursor-pointer select-none hover:text-gray-700 dark:hover:text-gray-300 transition-colors',
                )}
                style={col.width === 'flex-1' ? {} : undefined}
                onClick={col.sortable ? () => handleHeaderClick(col.key as SortColumn) : undefined}
                onKeyDown={
                  col.sortable
                    ? (e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault()
                          handleHeaderClick(col.key as SortColumn)
                        }
                      }
                    : undefined
                }
                tabIndex={col.sortable ? 0 : undefined}
              >
                <span className="inline-flex items-center gap-1">
                  {col.label}
                  {col.sortable &&
                    sortColumn === col.key &&
                    (sortDir === 'asc' ? (
                      <ArrowUp className="h-3 w-3 text-gray-500 dark:text-gray-400" />
                    ) : (
                      <ArrowDown className="h-3 w-3 text-gray-500 dark:text-gray-400" />
                    ))}
                </span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((session) => {
            const group = session.agentState.group
            const isSelected = session.id === selectedId
            const contextPercent = getContextPercent(session)
            const activityText =
              session.currentActivity || cleanPreviewText(session.lastUserMessage) || '--'

            return (
              <tr
                key={session.id}
                data-session-id={session.id}
                onClick={() => onSelect(session.id)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    onSelect(session.id)
                  }
                }}
                tabIndex={0}
                className={cn(
                  'border-b border-gray-200/50 dark:border-gray-800/50 transition-colors cursor-pointer',
                  isSelected
                    ? 'bg-indigo-500/10 border-l-2 border-l-indigo-500'
                    : 'border-l-2 border-l-transparent hover:bg-gray-100/50 dark:hover:bg-gray-800/50',
                )}
              >
                {/* Status */}
                <td className="px-2 py-2 w-[40px]">
                  <div className="flex items-center justify-center">
                    <StatusDot group={group} size="sm" pulse={group === 'autonomous'} />
                  </div>
                </td>

                {/* Project */}
                <td className="px-2 py-2 w-[140px]">
                  <span className="inline-flex items-center gap-1.5 max-w-full">
                    <FolderOpen className="h-3 w-3 text-amber-500 dark:text-amber-400 shrink-0" />
                    <span className="text-xs font-medium text-gray-800 dark:text-gray-200 truncate">
                      {session.projectDisplayName || session.project}
                    </span>
                  </span>
                </td>

                {/* Branch */}
                <td className="px-2 py-2 w-[120px]">
                  {session.effectiveBranch ? (
                    <span className="inline-flex items-center gap-1 max-w-full">
                      <span className="inline-flex items-center gap-1 bg-violet-50 dark:bg-violet-950/50 border border-violet-200 dark:border-violet-800 text-violet-700 dark:text-violet-300 px-1.5 py-0.5 rounded text-xs font-mono max-w-full">
                        <GitBranch className="h-3 w-3 shrink-0" />
                        <span className="truncate">{session.effectiveBranch}</span>
                      </span>
                    </span>
                  ) : (
                    <span className="text-xs text-gray-300 dark:text-gray-600">--</span>
                  )}
                </td>

                {/* Activity */}
                <td className="px-2 py-2">
                  {session.agentState.group === 'needs_you' ? (
                    <StateBadge agentState={session.agentState} />
                  ) : (
                    <span className="text-xs text-gray-700 dark:text-gray-300 truncate block">
                      {activityText}
                    </span>
                  )}
                </td>

                {/* Turns */}
                <td className="px-2 py-2 w-[60px]">
                  <span className="text-xs text-gray-700 dark:text-gray-300 tabular-nums">
                    {session.turnCount}
                  </span>
                </td>

                {/* Cost */}
                <td className="px-2 py-2 w-[70px]">
                  <span className="text-xs text-gray-700 dark:text-gray-300 tabular-nums">
                    {formatCost(session)}
                  </span>
                </td>

                {/* Context% */}
                <td className="px-2 py-2 w-[65px]">
                  {contextPercent != null ? (
                    <ContextBar percent={contextPercent} />
                  ) : (
                    <span className="text-[10px] text-zinc-400 dark:text-zinc-500">&mdash;</span>
                  )}
                </td>

                {/* Last Active */}
                <td className="px-2 py-2 w-[90px]">
                  <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
                    {session.lastActivityAt > 0 ? formatRelativeTime(session.lastActivityAt) : '--'}
                  </span>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
