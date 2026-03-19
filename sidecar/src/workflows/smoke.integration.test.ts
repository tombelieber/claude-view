import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { workflowSchema } from './workflow-schema.js'
import { buildAgentDefinitions } from './build-agents.js'
import { buildCoordinatorPrompt } from './coordinator-prompt.js'

const FIXTURE_PATH = resolve(import.meta.dirname, 'fixtures/hello-world.json')
const fixture = JSON.parse(readFileSync(FIXTURE_PATH, 'utf-8'))

describe('workflow infrastructure smoke test', () => {
  it('hello-world fixture passes Zod validation', () => {
    const result = workflowSchema.safeParse(fixture)
    expect(result.success).toBe(true)
  })

  it('builds exactly 1 agent definition from 1 stage node', () => {
    const wf = workflowSchema.parse(fixture)
    const agents = buildAgentDefinitions(wf, {})
    expect(Object.keys(agents)).toEqual(['stage-hello'])
    expect(agents['stage-hello'].prompt).toBe(
      "Say 'Hello from workflow!' and nothing else. Do not use any tools.",
    )
    expect(agents['stage-hello'].model).toBe('haiku')
    expect(agents['stage-hello'].maxTurns).toBe(3)
  })

  it('builds coordinator prompt with correct execution order', () => {
    const wf = workflowSchema.parse(fixture)
    const prompt = buildCoordinatorPrompt(wf, {})
    expect(prompt).toContain('stage-hello')
    expect(prompt).toContain('workflow-hello-world-run-')
    expect(prompt).toContain('1. stage-hello')
  })
})

// LIVE SDK TEST — skip if no API key
const hasApiKey = !!process.env.ANTHROPIC_API_KEY || !!process.env.CLAUDE_CODE_AUTH
describe.skipIf(!hasApiKey)('workflow live run (requires API key)', () => {
  it('runs hello-world workflow to completion', async () => {
    const { runWorkflow } = await import('./workflow-runner.js')
    const { SessionRegistry } = await import('../session-registry.js')
    const wf = workflowSchema.parse(fixture)
    const registry = new SessionRegistry()
    const response = await runWorkflow(wf, {}, registry)
    expect(response.teamName).toContain('workflow-hello-world-run-')
    expect(response.controlId).toBeDefined()
    expect(response.sessionId).toBeDefined()
  }, 60_000)
})
