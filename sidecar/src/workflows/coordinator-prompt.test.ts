import { describe, expect, it } from 'vitest'
import { buildCoordinatorPrompt } from './coordinator-prompt.js'
import type { WorkflowDefinition } from './workflow-schema.js'

describe('buildCoordinatorPrompt', () => {
  const wf: WorkflowDefinition = {
    id: 'test-flow',
    name: 'Test',
    description: '',
    author: '',
    version: '1.0.0',
    category: '',
    inputs: [{ name: 'path', type: 'file' }],
    defaults: { model: 'claude-sonnet-4-6' },
    nodes: [
      { id: 'input-0', type: 'workflowInput', position: { x: 0, y: 0 }, data: { label: 'path' } },
      {
        id: 'stage-a',
        type: 'workflowStage',
        position: { x: 100, y: 0 },
        data: { label: 'A', prompt: 'Do A' },
      },
      {
        id: 'stage-b',
        type: 'workflowStage',
        position: { x: 200, y: 0 },
        data: { label: 'B', prompt: 'Do B' },
      },
      { id: 'done-0', type: 'workflowDone', position: { x: 300, y: 0 }, data: { label: 'Done' } },
    ],
    edges: [
      { id: 'e1', source: 'input-0', target: 'stage-a' },
      { id: 'e2', source: 'stage-a', target: 'stage-b' },
      { id: 'e3', source: 'stage-b', target: 'done-0' },
    ],
  }

  it('includes correct topological order', () => {
    const prompt = buildCoordinatorPrompt(wf, { path: '/plan.md' })
    const aIdx = prompt.indexOf('stage-a')
    const bIdx = prompt.indexOf('stage-b')
    expect(aIdx).toBeLessThan(bIdx)
  })

  it('includes all inputs', () => {
    const prompt = buildCoordinatorPrompt(wf, { path: '/plan.md' })
    expect(prompt).toContain('/plan.md')
  })

  it('includes team name with workflow ID', () => {
    const prompt = buildCoordinatorPrompt(wf, {})
    expect(prompt).toContain('workflow-test-flow-run-')
  })

  it('throws on empty stages', () => {
    const empty: WorkflowDefinition = { ...wf, nodes: [wf.nodes[0], wf.nodes[3]], edges: [] }
    expect(() => buildCoordinatorPrompt(empty, {})).toThrow()
  })

  it('throws when required input is missing from inputValues', () => {
    const wfWithVar: WorkflowDefinition = {
      ...wf,
      nodes: [
        wf.nodes[0],
        {
          id: 'stage-a',
          type: 'workflowStage',
          position: { x: 100, y: 0 },
          data: { label: 'A', prompt: 'Audit {{path}}' },
        },
        wf.nodes[3],
      ],
      edges: [
        { id: 'e1', source: 'input-0', target: 'stage-a' },
        { id: 'e2', source: 'stage-a', target: 'done-0' },
      ],
    }
    expect(() => buildCoordinatorPrompt(wfWithVar, {})).toThrow('Missing input')
  })
})
