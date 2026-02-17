import { useState, useEffect, useCallback } from 'react'
import { X, Terminal, Users, BarChart3, DollarSign, GitBranch } from 'lucide-react'
import type { LiveSession } from './use-live-sessions'
import { RichTerminalPane } from './RichTerminalPane'
import { SwimLanes } from './SwimLanes'
import { SubAgentDrillDown } from './SubAgentDrillDown'
import { TimelineView } from './TimelineView'
import { CostBreakdown } from './CostBreakdown'
import { cn } from '../../lib/utils'

type TabId = 'terminal' | 'sub-agents' | 'timeline' | 'cost'

const TABS: { id: TabId; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { id: 'terminal', label: 'Terminal', icon: Terminal },
  { id: 'sub-agents', label: 'Sub-Agents', icon: Users },
  { id: 'timeline', label: 'Timeline', icon: BarChart3 },
  { id: 'cost', label: 'Cost', icon: DollarSign },
]

interface KanbanSidePanelProps {
  session: LiveSession
  onClose: () => void
}

export function KanbanSidePanel({ session, onClose }: KanbanSidePanelProps) {
  const hasSubAgents = session.subAgents && session.subAgents.length > 0
  const [activeTab, setActiveTab] = useState<TabId>(hasSubAgents ? 'sub-agents' : 'terminal')
  const [verboseMode] = useState(false)

  // Sub-agent drill-down state
  const [drillDownAgent, setDrillDownAgent] = useState<{
    agentId: string; agentType: string; description: string
  } | null>(null)

  // Reset tab and drill-down when session changes
  useEffect(() => {
    const newHasSubAgents = session.subAgents && session.subAgents.length > 0
    setActiveTab(newHasSubAgents ? 'sub-agents' : 'terminal')
    setDrillDownAgent(null)
  }, [session.id])

  // Escape to close (or exit drill-down first)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (drillDownAgent) {
          setDrillDownAgent(null)
        } else {
          onClose()
        }
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [onClose, drillDownAgent])

  const handleDrillDown = useCallback((agentId: string, agentType: string, description: string) => {
    setDrillDownAgent({ agentId, agentType, description })
  }, [])

  return (
    <div className="flex flex-col h-full bg-gray-950 border border-gray-800 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-800 bg-gray-900">
        <span className="text-sm font-medium text-gray-100 truncate">{session.projectDisplayName || session.project}</span>
        {session.gitBranch && (
          <span className="inline-flex items-center gap-1 text-xs font-mono text-gray-500 truncate max-w-[160px]">
            <GitBranch className="w-3 h-3 flex-shrink-0" />
            {session.gitBranch}
          </span>
        )}
        <div className="flex-1" />
        <span className="text-xs font-mono text-gray-400 tabular-nums">${session.cost.totalUsd.toFixed(2)}</span>
        <span className="text-xs text-gray-500">{session.turnCount} turns</span>
        <button
          onClick={onClose}
          aria-label="Close side panel"
          className="text-gray-500 hover:text-gray-300 transition-colors p-1"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-gray-800" role="tablist">
        {TABS.map((tab) => {
          const Icon = tab.icon
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={activeTab === tab.id}
              onClick={() => { setActiveTab(tab.id); setDrillDownAgent(null) }}
              className={cn(
                'flex items-center gap-1.5 px-4 py-2.5 text-xs font-medium transition-colors border-b-2',
                activeTab === tab.id
                  ? 'border-indigo-500 text-indigo-400'
                  : 'border-transparent text-gray-500 hover:text-gray-400',
              )}
            >
              <Icon className="w-3.5 h-3.5" />
              {tab.label}
            </button>
          )
        })}
      </div>

      {/* Tab content */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {activeTab === 'terminal' && (
          <RichTerminalPane
            sessionId={session.id}
            isVisible={true}
            verboseMode={verboseMode}
          />
        )}

        {activeTab === 'sub-agents' && (
          <div className="p-4 overflow-y-auto h-full">
            {drillDownAgent ? (
              <SubAgentDrillDown
                key={drillDownAgent.agentId}
                sessionId={session.id}
                agentId={drillDownAgent.agentId}
                agentType={drillDownAgent.agentType}
                description={drillDownAgent.description}
                onClose={() => setDrillDownAgent(null)}
              />
            ) : hasSubAgents ? (
              <SwimLanes
                subAgents={session.subAgents!}
                sessionActive={session.status === 'working'}
                onDrillDown={handleDrillDown}
              />
            ) : (
              <p className="text-sm text-gray-500 text-center py-8">No sub-agents in this session</p>
            )}
          </div>
        )}

        {activeTab === 'timeline' && (
          <div className="p-4 overflow-y-auto h-full">
            {hasSubAgents && session.startedAt ? (
              <TimelineView
                subAgents={session.subAgents!}
                sessionStartedAt={session.startedAt}
                sessionDurationMs={
                  session.status === 'done'
                    ? (session.lastActivityAt - session.startedAt) * 1000
                    : Date.now() - session.startedAt * 1000
                }
              />
            ) : (
              <p className="text-sm text-gray-500 text-center py-8">No timeline data available</p>
            )}
          </div>
        )}

        {activeTab === 'cost' && (
          <CostBreakdown cost={session.cost} subAgents={session.subAgents} />
        )}
      </div>
    </div>
  )
}
