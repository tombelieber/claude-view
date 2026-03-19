import { describe, expect, it } from 'vitest'
import { buildAgentDefinitions } from './build-agents.js'
import { buildCoordinatorPrompt } from './coordinator-prompt.js'
import type { WorkflowDefinition } from './workflow-schema.js'

const testWorkflow: WorkflowDefinition = {
  id: 'monitor-test',
  name: 'Monitor Test',
  description: '',
  author: '',
  version: '1.0.0',
  category: '',
  inputs: [],
  defaults: { model: 'claude-sonnet-4-6' },
  nodes: [
    {
      id: 'stage-a',
      type: 'workflowStage',
      position: { x: 0, y: 0 },
      data: { label: 'A', prompt: 'Do A', tools: ['Read'] },
    },
    {
      id: 'stage-b',
      type: 'workflowStage',
      position: { x: 300, y: 0 },
      data: { label: 'B', prompt: 'Do B' },
    },
    {
      id: 'done-0',
      type: 'workflowDone',
      position: { x: 600, y: 0 },
      data: { label: 'Done' },
    },
  ],
  edges: [
    { id: 'e1', source: 'stage-a', target: 'stage-b' },
    { id: 'e2', source: 'stage-b', target: 'done-0' },
  ],
}

describe('team visibility for Live Monitor', () => {
  it('coordinator prompt contains workflow ID prefix', () => {
    const prompt = buildCoordinatorPrompt(testWorkflow, {})
    expect(prompt).toContain('workflow-monitor-test-run-')
    expect(prompt).toContain('stage-a')
    expect(prompt).toContain('stage-b')
  })

  it('agent definitions contain correct stage IDs as keys', () => {
    const agents = buildAgentDefinitions(testWorkflow, {})
    expect(Object.keys(agents)).toEqual(['stage-a', 'stage-b'])
    expect(agents['stage-a'].tools).toEqual(['Read'])
  })

  it('agent definitions include description for team member display', () => {
    const agents = buildAgentDefinitions(testWorkflow, {})
    expect(agents['stage-a'].description).toContain('Workflow stage: A')
  })

  it('coordinator prompt includes execution order', () => {
    const prompt = buildCoordinatorPrompt(testWorkflow, {})
    // stage-a before stage-b due to edge ordering
    const stageAPos = prompt.indexOf('stage-a')
    const stageBPos = prompt.indexOf('stage-b')
    expect(stageAPos).toBeLessThan(stageBPos)
  })

  it('agent definitions exclude done nodes', () => {
    const agents = buildAgentDefinitions(testWorkflow, {})
    expect(agents['done-0']).toBeUndefined()
  })
})
