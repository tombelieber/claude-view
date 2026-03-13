import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { WorkflowDetail } from '../types/generated/WorkflowDetail'
import type { WorkflowSummary } from '../types/generated/WorkflowSummary'

async function fetchWorkflows(): Promise<WorkflowSummary[]> {
  const res = await fetch('/api/workflows')
  if (!res.ok) throw new Error('Failed to fetch workflows')
  return res.json()
}

async function fetchWorkflow(id: string): Promise<WorkflowDetail> {
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
  return useQuery({ queryKey: ['workflows'], queryFn: fetchWorkflows, staleTime: 60_000 })
}

export function useWorkflow(id: string) {
  return useQuery({
    queryKey: ['workflows', id],
    queryFn: () => fetchWorkflow(id),
    enabled: !!id,
    staleTime: 60_000,
  })
}

export function useCreateWorkflow() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: createWorkflow,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['workflows'] }),
  })
}

export function useDeleteWorkflow() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: deleteWorkflow,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['workflows'] }),
  })
}
