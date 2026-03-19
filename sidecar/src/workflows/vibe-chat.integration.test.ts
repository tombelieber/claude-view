import { describe, expect, it } from 'vitest'
import { extractLastJsonBlock } from './json-extractor.js'
import { workflowSchema } from './workflow-schema.js'

describe('vibe chat JSON extraction flow', () => {
  it('extracts valid workflow JSON from assistant response', () => {
    const assistantText = [
      "Here's your workflow:\n",
      '```json',
      JSON.stringify({
        id: 'test',
        name: 'Test',
        description: 'A test',
        author: 'user',
        version: '1.0.0',
        category: 'Dev',
        inputs: [],
        defaults: { model: 'claude-sonnet-4-6' },
        nodes: [
          {
            id: 'done-0',
            type: 'workflowDone',
            position: { x: 0, y: 0 },
            data: { label: 'Done' },
          },
        ],
        edges: [],
      }),
      '```',
      "\nI've created a simple workflow.",
    ].join('\n')

    const json = extractLastJsonBlock(assistantText)
    expect(json).not.toBeNull()
    const parsed = JSON.parse(json!)
    expect(workflowSchema.safeParse(parsed).success).toBe(true)
  })

  it('handles invalid JSON -- returns parseable but schema-invalid', () => {
    const badAssistant = '```json\n{"id":"test"}\n```'
    const json = extractLastJsonBlock(badAssistant)
    expect(json).not.toBeNull()
    expect(workflowSchema.safeParse(JSON.parse(json!)).success).toBe(false)
  })

  it('returns null when no JSON block in response', () => {
    expect(extractLastJsonBlock('I need more information.')).toBeNull()
  })

  it('picks the last JSON block when multiple exist', () => {
    const multiBlock = [
      '```json\n{"id":"first"}\n```',
      'Some text in between.',
      '```json\n{"id":"second"}\n```',
    ].join('\n')

    const json = extractLastJsonBlock(multiBlock)
    expect(json).not.toBeNull()
    expect(JSON.parse(json!).id).toBe('second')
  })
})
