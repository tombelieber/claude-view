import { describe, expect, it } from 'vitest'
import { workflowSchema } from './workflow-schema.js'

const GOLDEN_V1 = {
  id: 'golden-v1',
  name: 'Golden V1',
  description: 'First stable schema',
  author: 'test',
  version: '1.0.0',
  category: 'Dev',
  inputs: [{ name: 'path', type: 'file', description: 'Plan' }],
  defaults: {
    model: 'claude-sonnet-4-6',
    permissionMode: 'default',
    settingSources: ['user', 'project'],
    effort: 'high',
    maxBudgetUsd: 20,
    maxTurns: 200,
  },
  nodes: [
    {
      id: 'input-0',
      type: 'workflowInput',
      position: { x: 0, y: 0 },
      data: { label: 'path' },
    },
    {
      id: 'stage-1',
      type: 'workflowStage',
      position: { x: 300, y: 0 },
      data: {
        label: 'Audit',
        prompt: 'Audit {{path}}',
        model: 'opus',
        tools: ['Read'],
        skills: ['/prove-it'],
        maxTurns: 100,
        gate: {
          type: 'json_field',
          field: 'verdict',
          operator: 'equals',
          value: 'PASS',
          maxRetries: 3,
        },
      },
    },
    {
      id: 'done-0',
      type: 'workflowDone',
      position: { x: 600, y: 0 },
      data: { label: 'Done' },
    },
  ],
  edges: [
    { id: 'e1', source: 'input-0', target: 'stage-1' },
    { id: 'e2', source: 'stage-1', target: 'done-0' },
  ],
}

describe('schema compatibility', () => {
  it('golden V1 still parses successfully', () => {
    expect(workflowSchema.safeParse(GOLDEN_V1).success).toBe(true)
  })

  it('golden V1 round-trips through JSON serialization', () => {
    const parsed = workflowSchema.parse(GOLDEN_V1)
    expect(workflowSchema.safeParse(JSON.parse(JSON.stringify(parsed))).success).toBe(true)
  })

  it('golden V1 preserves all gate fields', () => {
    const parsed = workflowSchema.parse(GOLDEN_V1)
    const stageNode = parsed.nodes.find((n) => n.type === 'workflowStage')
    if (stageNode?.type === 'workflowStage') {
      expect(stageNode.data.gate).toEqual({
        type: 'json_field',
        field: 'verdict',
        operator: 'equals',
        value: 'PASS',
        maxRetries: 3,
      })
    }
  })
})
