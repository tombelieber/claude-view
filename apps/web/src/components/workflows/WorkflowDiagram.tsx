import {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  Handle,
  type Node,
  Position,
  ReactFlow,
  useEdgesState,
  useNodesState,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { useEffect } from 'react'
import type { WorkflowDefinition } from '../../types/generated/WorkflowDefinition'
import type { StageStatus } from './WorkflowStageColumn'

// ─── Types ────────────────────────────────────────────────────────────────────

type NodeKind = 'input' | 'stage' | 'done'

interface StageNodeData {
  kind: NodeKind
  label: string
  skills?: string[]
  parallel?: boolean
  gate?: string
  status?: StageStatus
  [key: string]: unknown
}

// ─── Layout constants ─────────────────────────────────────────────────────────

const NODE_W = 180
const NODE_H_BASE = 72
const NODE_H_PER_SKILL = 20
const X_GAP = 72

function nodeHeight(skills: string[] = []): number {
  return NODE_H_BASE + Math.max(0, skills.length - 1) * NODE_H_PER_SKILL
}

// ─── Build nodes + edges from definition ─────────────────────────────────────

export function buildFlow(
  def: WorkflowDefinition,
  stageStatuses?: Map<string, StageStatus>,
): { nodes: Node<StageNodeData>[]; edges: Edge[] } {
  const nodes: Node<StageNodeData>[] = []
  const edges: Edge[] = []

  let x = 0

  // Input node
  nodes.push({
    id: 'input',
    type: 'workflowNode',
    position: { x, y: 0 },
    data: { kind: 'input', label: def.inputs[0]?.name ?? 'Input' },
    draggable: false,
  })
  x += NODE_W + X_GAP

  // Stage nodes
  for (const stage of def.stages) {
    nodes.push({
      id: stage.name,
      type: 'workflowNode',
      position: { x, y: 0 },
      data: {
        kind: 'stage',
        label: stage.name,
        skills: stage.skills,
        parallel: stage.parallel,
        gate: stage.gate?.condition,
        status: stageStatuses?.get(stage.name),
      },
      draggable: false,
    })
    x += NODE_W + X_GAP
  }

  // Done node
  nodes.push({
    id: 'done',
    type: 'workflowNode',
    position: { x, y: 0 },
    data: { kind: 'done', label: 'Done' },
    draggable: false,
  })

  const edgeBase = {
    type: 'default',
    style: { stroke: '#AEAEB2', strokeWidth: 1.5 },
    zIndex: 10,
  }

  // Edges: input → first stage
  if (def.stages.length > 0) {
    edges.push({
      ...edgeBase,
      id: 'e-input-0',
      source: 'input',
      target: def.stages[0].name,
    })
  }

  // Edges: stage → next (or done), retry loop
  for (let i = 0; i < def.stages.length; i++) {
    const stage = def.stages[i]
    const next = def.stages[i + 1]
    const targetId = next ? next.name : 'done'

    edges.push({
      ...edgeBase,
      id: `e-${stage.name}-pass`,
      source: stage.name,
      target: targetId,
      label: stage.gate ? 'pass' : undefined,
      labelStyle: { fontSize: 10, fill: '#AEAEB2', fontFamily: 'var(--font-sans)' },
      labelBgPadding: [4, 2] as [number, number],
      labelBgStyle: { fill: 'transparent' },
    })

    if (stage.gate?.retry) {
      edges.push({
        id: `e-${stage.name}-retry`,
        source: stage.name,
        target: stage.name,
        sourceHandle: null,
        targetHandle: null,
        label: 'retry',
        labelStyle: { fontSize: 10, fill: '#EF4444', fontFamily: 'var(--font-sans)' },
        labelBgPadding: [4, 2] as [number, number],
        labelBgStyle: { fill: 'transparent' },
        type: 'default',
        style: { stroke: '#EF4444', strokeWidth: 1.5, strokeDasharray: '4 3' },
        zIndex: 10,
      })
    }
  }

  // Vertically center all nodes relative to tallest
  const maxH = Math.max(...nodes.map((n) => nodeHeight((n.data as StageNodeData).skills)))
  for (const node of nodes) {
    const h = nodeHeight((node.data as StageNodeData).skills)
    node.position.y = (maxH - h) / 2
  }

  return { nodes, edges }
}

// ─── Status styles ────────────────────────────────────────────────────────────

const STATUS_RING: Record<StageStatus, string> = {
  locked: '',
  running: 'ring-2 ring-[#3B82F6]/30',
  passed: 'ring-2 ring-[#22C55E]/30',
  failed: 'ring-2 ring-[#EF4444]/30',
}

const STATUS_BORDER: Record<StageStatus, string> = {
  locked: 'border-[#E5E5EA] dark:border-[#3A3A3C]',
  running: 'border-[#3B82F6]',
  passed: 'border-[#22C55E]',
  failed: 'border-[#EF4444]',
}

const STATUS_BG: Record<StageStatus, string> = {
  locked: 'bg-white dark:bg-[#1C1C1E]',
  running: 'bg-[#EFF6FF] dark:bg-[#1D3461]',
  passed: 'bg-[#F0FDF4] dark:bg-[#052E16]',
  failed: 'bg-[#FEF2F2] dark:bg-[#450A0A]',
}

// ─── Custom node ──────────────────────────────────────────────────────────────

function WorkflowNodeComponent({ data }: { data: StageNodeData }) {
  const { kind, label, skills, parallel, gate, status } = data

  // Handle styles — invisible, just connection points
  const handleStyle: React.CSSProperties = {
    background: 'transparent',
    border: 'none',
    width: 8,
    height: 8,
  }

  if (kind === 'input') {
    return (
      <div className="relative flex items-center justify-center px-4 py-2.5 rounded-full bg-white dark:bg-[#1C1C1E] border border-[#D1D1D6] dark:border-[#3A3A3C] shadow-sm">
        <Handle type="source" position={Position.Right} style={handleStyle} />
        <span className="text-[12px] font-semibold text-[#1D1D1F] dark:text-white whitespace-nowrap">
          {label}
        </span>
      </div>
    )
  }

  if (kind === 'done') {
    return (
      <div className="relative flex items-center justify-center px-4 py-2.5 rounded-full bg-[#F5F5F7] dark:bg-[#2C2C2E] border border-[#D1D1D6] dark:border-[#3A3A3C]">
        <Handle type="target" position={Position.Left} style={handleStyle} />
        <span className="text-[12px] font-medium text-[#6E6E73] dark:text-[#98989D] whitespace-nowrap">
          {label}
        </span>
      </div>
    )
  }

  // Stage node
  const effectiveStatus: StageStatus = status ?? 'locked'
  const ringClass = STATUS_RING[effectiveStatus]
  const borderClass = STATUS_BORDER[effectiveStatus]
  const bgClass = STATUS_BG[effectiveStatus]

  return (
    <div
      className={`relative flex flex-col rounded-xl border shadow-sm transition-all duration-200 overflow-hidden ${bgClass} ${borderClass} ${ringClass}`}
      style={{ width: NODE_W }}
    >
      <Handle type="target" position={Position.Left} style={handleStyle} />
      <Handle type="source" position={Position.Right} style={handleStyle} />

      {/* Header */}
      <div className="px-3 pt-3 pb-2">
        <div className="flex items-start justify-between gap-1.5">
          <span className="text-[13px] font-semibold text-[#1D1D1F] dark:text-white leading-tight">
            {label}
          </span>
          <div className="flex items-center gap-1 shrink-0 mt-0.5">
            {parallel && (
              <span className="text-[9px] px-1 py-0.5 rounded bg-[#22C55E]/10 text-[#22C55E] font-semibold">
                ⇉
              </span>
            )}
            {effectiveStatus === 'running' && (
              <span className="w-1.5 h-1.5 rounded-full bg-[#3B82F6] animate-pulse shrink-0" />
            )}
            {effectiveStatus === 'passed' && (
              <span className="text-[#22C55E] text-[12px] font-bold leading-none">✓</span>
            )}
            {effectiveStatus === 'failed' && (
              <span className="text-[#EF4444] text-[12px] font-bold leading-none">✕</span>
            )}
          </div>
        </div>
        {gate && (
          <p className="text-[10px] text-[#6E6E73] dark:text-[#98989D] mt-0.5 truncate leading-tight">
            {gate}
          </p>
        )}
      </div>

      {/* Skills */}
      {skills && skills.length > 0 && (
        <div className="px-3 pb-3 flex flex-col gap-1">
          {skills.map((skill) => (
            <div
              key={skill}
              className="text-[11px] px-1.5 py-0.5 rounded-md bg-black/[0.04] dark:bg-white/[0.06] text-[#6E6E73] dark:text-[#98989D] truncate"
            >
              {skill}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// Stable nodeTypes — defined outside component to prevent re-registration
const nodeTypes = { workflowNode: WorkflowNodeComponent }

// ─── Main component ───────────────────────────────────────────────────────────

interface WorkflowDiagramProps {
  definition: WorkflowDefinition
  stageStatuses?: Map<string, StageStatus>
  isDark?: boolean
}

export function WorkflowDiagram({
  definition,
  stageStatuses,
  isDark = false,
}: WorkflowDiagramProps) {
  const { nodes: initNodes, edges: initEdges } = buildFlow(definition, stageStatuses)
  const [nodes, setNodes, onNodesChange] = useNodesState(initNodes)
  const [edges, setEdges, onEdgesChange] = useEdgesState(initEdges)

  useEffect(() => {
    const { nodes: n, edges: e } = buildFlow(definition, stageStatuses)
    setNodes(n)
    setEdges(e)
  }, [definition, stageStatuses, setNodes, setEdges])

  return (
    <div className="w-full h-full" style={{ minHeight: 240 }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.35, maxZoom: 1.0 }}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={false}
        colorMode={isDark ? 'dark' : 'light'}
        proOptions={{ hideAttribution: true }}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={20}
          size={1}
          color={isDark ? '#3A3A3C' : '#E5E5EA'}
        />
        <Controls showInteractive={false} style={{ boxShadow: 'none' }} />
      </ReactFlow>
    </div>
  )
}
