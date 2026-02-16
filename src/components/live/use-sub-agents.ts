import { useMemo } from 'react'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

export interface UseSubAgentsResult {
  all: SubAgentInfo[]
  active: SubAgentInfo[]
  completed: SubAgentInfo[]
  errored: SubAgentInfo[]
  totalCost: number
  activeCount: number
  isAnyRunning: boolean
}

export function useSubAgents(subAgents: SubAgentInfo[]): UseSubAgentsResult {
  return useMemo(() => {
    const active = subAgents.filter(a => a.status === 'running')
    const completed = subAgents.filter(a => a.status === 'complete')
    const errored = subAgents.filter(a => a.status === 'error')
    const totalCost = subAgents.reduce((sum, a) => sum + (a.costUsd ?? 0), 0)

    return {
      all: subAgents,
      active,
      completed,
      errored,
      totalCost,
      activeCount: active.length,
      isAnyRunning: active.length > 0,
    }
  }, [subAgents])
}
