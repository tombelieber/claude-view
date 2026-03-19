import { describe, expect, it } from 'vitest'
import { workflowSchema } from './workflow-schema.js'

describe('workflowSchema', () => {
  const validWorkflow = {
    id: 'test-flow',
    name: 'Test Flow',
    description: 'A test workflow',
    author: 'test',
    version: '1.0.0',
    category: 'Development',
    inputs: [{ name: 'plan_path', type: 'file', description: 'Path to plan' }],
    defaults: {
      model: 'claude-sonnet-4-6',
      permissionMode: 'default',
      settingSources: ['user', 'project'],
      effort: 'high',
      maxBudgetUsd: 20.0,
      maxTurns: 200,
    },
    nodes: [
      {
        id: 'input-0',
        type: 'workflowInput',
        position: { x: 0, y: 0 },
        data: { label: 'plan_path' },
      },
      {
        id: 'stage-1',
        type: 'workflowStage',
        position: { x: 300, y: 0 },
        data: {
          label: 'Audit',
          prompt: 'Run audit on {{plan_path}}',
          model: 'opus',
          tools: ['Read', 'Write'],
          skills: ['/prove-it'],
          maxTurns: 100,
          disallowedTools: [],
          gate: {
            type: 'json_field',
            field: 'verdict',
            operator: 'equals',
            value: 'PASS',
            maxRetries: 3,
          },
        },
      },
      { id: 'done-0', type: 'workflowDone', position: { x: 600, y: 0 }, data: { label: 'Done' } },
    ],
    edges: [
      { id: 'e1', source: 'input-0', target: 'stage-1' },
      { id: 'e2', source: 'stage-1', target: 'done-0' },
    ],
  }

  it('accepts a valid workflow', () => {
    const result = workflowSchema.safeParse(validWorkflow)
    expect(result.success).toBe(true)
  })

  it('rejects missing required fields', () => {
    const { id, ...noId } = validWorkflow
    expect(workflowSchema.safeParse(noId).success).toBe(false)
  })

  it('rejects invalid node types', () => {
    const bad = { ...validWorkflow, nodes: [{ ...validWorkflow.nodes[0], type: 'invalid' }] }
    expect(workflowSchema.safeParse(bad).success).toBe(false)
  })

  it('rejects edges referencing nonexistent nodes', () => {
    const bad = { ...validWorkflow, edges: [{ id: 'e-bad', source: 'nope', target: 'stage-1' }] }
    const result = workflowSchema.safeParse(bad)
    expect(result.success).toBe(false)
  })

  it('rejects invalid gate operator', () => {
    const bad = structuredClone(validWorkflow)
    ;(bad.nodes[1].data as any).gate.operator = 'invalid'
    expect(workflowSchema.safeParse(bad).success).toBe(false)
  })

  it('accepts gate type file_exists with path field', () => {
    const withFileGate = structuredClone(validWorkflow)
    ;(withFileGate.nodes[1].data as any).gate = {
      type: 'file_exists',
      path: '/tmp/out.md',
      maxRetries: 1,
    }
    expect(workflowSchema.safeParse(withFileGate).success).toBe(true)
  })

  it('accepts gate type regex with pattern field', () => {
    const withRegex = structuredClone(validWorkflow)
    ;(withRegex.nodes[1].data as any).gate = { type: 'regex', pattern: 'PASS', maxRetries: 2 }
    expect(workflowSchema.safeParse(withRegex).success).toBe(true)
  })

  it('allows workflowStage without gate (optional)', () => {
    const noGate = structuredClone(validWorkflow)
    delete (noGate.nodes[1].data as any).gate
    expect(workflowSchema.safeParse(noGate).success).toBe(true)
  })
})
