import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { ClaudeHomeEntry } from '../types/generated/ClaudeHomeEntry'
import type { WorkflowAgentDetail } from '../types/generated/WorkflowAgentDetail'
import type { WorkflowDetail } from '../types/generated/WorkflowDetail'
import type { WorkflowRunDetail } from '../types/generated/WorkflowRunDetail'
import type { WorkflowRunsResponse } from '../types/generated/WorkflowRunsResponse'
import type { WorkflowSummary } from '../types/generated/WorkflowSummary'

async function fetchWorkflowRuns(): Promise<WorkflowRunsResponse> {
  const res = await fetch('/api/workflows/runs')
  if (!res.ok) throw new Error('Failed to fetch workflow runs')
  return res.json()
}

async function fetchWorkflowRun(sessionId: string, runId: string): Promise<WorkflowRunDetail> {
  const res = await fetch(
    `/api/workflows/runs/${encodeURIComponent(sessionId)}/${encodeURIComponent(runId)}`,
  )
  if (!res.ok) throw new Error(`Workflow run ${runId} not found`)
  return res.json()
}

async function fetchWorkflowAgent(
  sessionId: string,
  runId: string,
  agentId: string,
): Promise<WorkflowAgentDetail> {
  const res = await fetch(
    `/api/workflows/runs/${encodeURIComponent(sessionId)}/${encodeURIComponent(runId)}/agents/${encodeURIComponent(agentId)}`,
  )
  if (!res.ok) throw new Error(`Workflow agent ${agentId} not found`)
  return res.json()
}

async function fetchClaudeHome(): Promise<ClaudeHomeEntry[]> {
  const res = await fetch('/api/claude-home')
  if (!res.ok) throw new Error('Failed to fetch Claude home entries')
  return res.json()
}

async function fetchWorkflowDefinitions(): Promise<WorkflowSummary[]> {
  const res = await fetch('/api/workflows')
  if (!res.ok) throw new Error('Failed to fetch workflows')
  return res.json()
}

async function fetchWorkflowDefinition(id: string): Promise<WorkflowDetail> {
  const res = await fetch(`/api/workflows/${id}`)
  if (!res.ok) throw new Error(`Workflow ${id} not found`)
  return res.json()
}

async function createWorkflow(yaml: string): Promise<WorkflowDetail> {
  const res = await fetch('/api/workflows', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ yaml }),
  })
  if (!res.ok) throw new Error('Failed to create workflow')
  return res.json()
}

async function deleteWorkflow(id: string): Promise<void> {
  const res = await fetch(`/api/workflows/${id}`, { method: 'DELETE' })
  if (!res.ok) throw new Error(`Failed to delete workflow ${id}`)
}

export function useWorkflows() {
  return useQuery({
    queryKey: ['workflow-definitions'],
    queryFn: fetchWorkflowDefinitions,
    staleTime: 60_000,
  })
}

export function useWorkflow(id: string) {
  return useQuery({
    queryKey: ['workflow-definitions', id],
    queryFn: () => fetchWorkflowDefinition(id),
    enabled: !!id,
    staleTime: 60_000,
  })
}

export function useWorkflowRuns() {
  return useQuery({
    queryKey: ['workflow-runs'],
    queryFn: fetchWorkflowRuns,
    refetchInterval: 10_000,
    staleTime: 5_000,
  })
}

export function useWorkflowRun(sessionId: string, runId: string) {
  return useQuery({
    queryKey: ['workflow-runs', sessionId, runId],
    queryFn: () => fetchWorkflowRun(sessionId, runId),
    enabled: !!sessionId && !!runId,
    refetchInterval: 10_000,
    staleTime: 5_000,
  })
}

export function useWorkflowAgent(sessionId: string, runId: string, agentId: string | null) {
  return useQuery({
    queryKey: ['workflow-runs', sessionId, runId, 'agents', agentId],
    queryFn: () => fetchWorkflowAgent(sessionId, runId, agentId ?? ''),
    enabled: !!sessionId && !!runId && !!agentId,
    refetchInterval: 10_000,
    staleTime: 5_000,
  })
}

export function useClaudeHomeEntries() {
  return useQuery({
    queryKey: ['claude-home'],
    queryFn: fetchClaudeHome,
    refetchInterval: 30_000,
    staleTime: 10_000,
  })
}

export function useCreateWorkflow() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: createWorkflow,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['workflow-definitions'] }),
  })
}

export function useDeleteWorkflow() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: deleteWorkflow,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['workflow-definitions'] }),
  })
}
