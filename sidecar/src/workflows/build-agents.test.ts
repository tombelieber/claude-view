import { describe, expect, it } from 'vitest'
import { buildAgentDefinitions } from './build-agents.js'
import type { WorkflowDefinition } from './workflow-schema.js'

const makeWorkflow = (nodes: WorkflowDefinition['nodes']): WorkflowDefinition => ({
  id: 'test',
  name: 'Test',
  description: '',
  author: '',
  version: '1.0.0',
  category: '',
  inputs: [{ name: 'path', type: 'file' }],
  defaults: { model: 'claude-sonnet-4-6' },
  nodes,
  edges: [],
})

describe('buildAgentDefinitions', () => {
  it('maps workflowStage nodes to AgentDefinition', () => {
    const wf = makeWorkflow([
      {
        id: 'stage-1',
        type: 'workflowStage',
        position: { x: 0, y: 0 },
        data: {
          label: 'Audit',
          prompt: 'Do {{path}}',
          model: 'opus',
          tools: ['Read'],
          skills: [],
          maxTurns: 50,
        },
      },
    ])
    const agents = buildAgentDefinitions(wf, { path: '/plan.md' })
    expect(agents['stage-1']).toBeDefined()
    expect(agents['stage-1'].prompt).toBe('Do /plan.md')
    expect(agents['stage-1'].model).toBe('opus')
    expect(agents['stage-1'].tools).toEqual(['Read'])
  })

  it('skips input and done nodes', () => {
    const wf = makeWorkflow([
      { id: 'input-0', type: 'workflowInput', position: { x: 0, y: 0 }, data: { label: 'x' } },
      { id: 'done-0', type: 'workflowDone', position: { x: 0, y: 0 }, data: { label: 'Done' } },
    ])
    const agents = buildAgentDefinitions(wf, {})
    expect(Object.keys(agents)).toEqual([])
  })

  it('passes model string through (no cast)', () => {
    const wf = makeWorkflow([
      {
        id: 's1',
        type: 'workflowStage',
        position: { x: 0, y: 0 },
        data: { label: 'X', prompt: 'Y', model: 'claude-opus-4-6' },
      },
    ])
    const agents = buildAgentDefinitions(wf, {})
    expect(agents['s1'].model).toBe('claude-opus-4-6')
  })

  it('returns empty for empty workflow', () => {
    const wf = makeWorkflow([])
    expect(buildAgentDefinitions(wf, {})).toEqual({})
  })
})
