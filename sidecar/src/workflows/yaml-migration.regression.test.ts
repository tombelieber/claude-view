import { describe, expect, it } from 'vitest'
import { workflowSchema } from './workflow-schema.js'
import { convertYamlToJson } from './yaml-to-json.js'

const GOLDEN_YAML = {
  name: 'Golden YAML',
  description: 'Legacy format',
  author: 'test',
  category: 'Dev',
  version: '1.0.0',
  inputs: [{ name: 'path', type: 'file', description: 'Plan' }],
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

describe('YAML migration regression', () => {
  it('golden YAML converts to valid JSON schema', () => {
    expect(workflowSchema.safeParse(convertYamlToJson(GOLDEN_YAML)).success).toBe(true)
  })

  it('round-trip: YAML to JSON to parse to validate', () => {
    const json = convertYamlToJson(GOLDEN_YAML)
    expect(workflowSchema.safeParse(JSON.parse(JSON.stringify(json))).success).toBe(true)
  })

  it('converted output has correct node count', () => {
    const json = convertYamlToJson(GOLDEN_YAML)
    // 1 input + 2 stages + 1 done = 4 nodes
    expect(json.nodes).toHaveLength(4)
  })

  it('converted output has correct edge chain', () => {
    const json = convertYamlToJson(GOLDEN_YAML)
    // input->audit, audit->ship, ship->done = 3 edges
    expect(json.edges).toHaveLength(3)
  })

  it('gate conversion is stable across invocations', () => {
    const first = convertYamlToJson(GOLDEN_YAML)
    const second = convertYamlToJson(GOLDEN_YAML)
    expect(JSON.stringify(first)).toBe(JSON.stringify(second))
  })
})
