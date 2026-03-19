import { describe, expect, it } from 'vitest'
import { workflowSchema } from './workflow-schema.js'
import { convertYamlToJson } from './yaml-to-json.js'

const YAML_FIXTURE = {
  name: 'Test Flow',
  description: 'A test',
  author: 'test',
  category: 'Dev',
  version: '1.0.0',
  inputs: [{ name: 'path', type: 'file', description: 'Plan path' }],
  stages: [
    {
      id: 'audit',
      name: 'Audit',
      prompt: 'Audit {{path}}',
      skills: ['/prove-it'],
      gate: { condition: 'verdict == PASS', retry: true },
    },
    { id: 'ship', name: 'Ship', prompt: 'Ship it', skills: ['/commit'] },
  ],
}

describe('convertYamlToJson', () => {
  it('converts valid YAML stages to nodes + edges', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    expect(json.nodes.filter((n) => n.type === 'workflowStage')).toHaveLength(2)
    expect(json.edges.length).toBeGreaterThanOrEqual(1)
    expect(json.edges.some((e) => e.source.includes('audit') && e.target.includes('ship'))).toBe(
      true,
    )
  })

  it('preserves metadata (name, author, etc.)', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    expect(json.name).toBe('Test Flow')
    expect(json.author).toBe('test')
    expect(json.category).toBe('Dev')
    expect(json.inputs).toEqual(YAML_FIXTURE.inputs)
  })

  it('converts old gate {condition, retry} to GateCondition', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    const auditNode = json.nodes.find((n) => n.id.includes('audit'))
    expect(auditNode?.type).toBe('workflowStage')
    if (auditNode?.type === 'workflowStage') {
      expect(auditNode.data.gate).toBeDefined()
      expect(auditNode.data.gate!.type).toBe('regex')
      expect(auditNode.data.gate!.maxRetries).toBeGreaterThan(0)
    }
  })

  it('handles YAML with no stages (empty)', () => {
    const json = convertYamlToJson({ ...YAML_FIXTURE, stages: [] })
    expect(json.nodes.filter((n) => n.type === 'workflowStage')).toHaveLength(0)
  })

  it('handles YAML with no gates', () => {
    const noGates = { ...YAML_FIXTURE, stages: [{ id: 's1', name: 'S1', prompt: 'Do' }] }
    const json = convertYamlToJson(noGates)
    const stage = json.nodes.find((n) => n.type === 'workflowStage')
    if (stage?.type === 'workflowStage') {
      expect(stage.data.gate).toBeUndefined()
    }
  })

  it('produces valid JSON that passes Zod schema', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    const result = workflowSchema.safeParse(json)
    expect(result.success).toBe(true)
  })

  it('generates stable id from name', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    expect(json.id).toBe('test-flow')
  })

  it('sets default model in defaults', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    expect(json.defaults.model).toBeDefined()
  })

  it('creates done node', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    expect(json.nodes.some((n) => n.type === 'workflowDone')).toBe(true)
  })

  it('creates input nodes from inputs', () => {
    const json = convertYamlToJson(YAML_FIXTURE)
    expect(json.nodes.filter((n) => n.type === 'workflowInput')).toHaveLength(1)
  })
})
