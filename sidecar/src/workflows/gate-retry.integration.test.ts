import { describe, expect, it, vi } from 'vitest'
import { createGateHook } from './workflow-runner.js'
import type { WorkflowDefinition } from './workflow-schema.js'

const testWorkflow: WorkflowDefinition = {
  id: 'test-flow',
  name: 'Test',
  description: '',
  author: '',
  version: '1.0.0',
  category: '',
  inputs: [],
  defaults: { model: 'claude-sonnet-4-6' },
  nodes: [
    {
      id: 'stage-audit',
      type: 'workflowStage',
      position: { x: 0, y: 0 },
      data: {
        label: 'Audit',
        prompt: 'Do audit',
        gate: {
          type: 'json_field',
          field: 'verdict',
          operator: 'equals',
          value: 'PASS',
          maxRetries: 3,
        },
      },
    },
  ],
  edges: [],
}

const failInput = {
  hook_event_name: 'PostToolUse',
  tool_name: 'Agent',
  tool_input: { name: 'stage-audit' },
  tool_response: '{"verdict":"FAIL"}',
  tool_use_id: 'tool-1',
  session_id: 'test-session',
  transcript_path: '/tmp/test',
  cwd: '/tmp',
}

describe('gate retry loop', () => {
  it('injects retry message on gate failure', async () => {
    const pushSpy = vi.fn()
    const gateHook = createGateHook(testWorkflow, pushSpy)

    await gateHook(failInput, 'tool-1', { signal: new AbortController().signal })

    expect(pushSpy).toHaveBeenCalledTimes(1)
    const msg = pushSpy.mock.calls[0][0]
    expect(msg.message.content[0].text).toContain('Gate FAILED for stage-audit (attempt 1/3)')
  })

  it('injects pass message on gate success', async () => {
    const pushSpy = vi.fn()
    const gateHook = createGateHook(testWorkflow, pushSpy)
    const passInput = { ...failInput, tool_response: '{"verdict":"PASS"}' }

    await gateHook(passInput, 'tool-1', { signal: new AbortController().signal })

    const msg = pushSpy.mock.calls[0][0]
    expect(msg.message.content[0].text).toContain('Gate PASSED for stage-audit')
  })

  it('marks stage failed after maxRetries exhausted', async () => {
    const pushSpy = vi.fn()
    const wf1: WorkflowDefinition = {
      ...testWorkflow,
      nodes: [
        {
          id: 'stage-audit',
          type: 'workflowStage',
          position: { x: 0, y: 0 },
          data: {
            label: 'Audit',
            prompt: 'Do audit',
            gate: {
              type: 'json_field',
              field: 'verdict',
              operator: 'equals',
              value: 'PASS',
              maxRetries: 1,
            },
          },
        },
      ],
    }
    const gateHook = createGateHook(wf1, pushSpy)

    // First call — attempt 1, maxRetries is 1 so no retries allowed
    await gateHook(failInput, 'tool-1', { signal: new AbortController().signal })

    const lastMsg = pushSpy.mock.calls[pushSpy.mock.calls.length - 1][0]
    expect(lastMsg.message.content[0].text).toContain('Workflow stage failed')
  })

  it('does nothing for non-Agent tools', async () => {
    const pushSpy = vi.fn()
    const gateHook = createGateHook(testWorkflow, pushSpy)
    const nonAgentInput = { ...failInput, tool_name: 'Read' }

    const result = await gateHook(nonAgentInput, 'tool-1', {
      signal: new AbortController().signal,
    })

    expect(pushSpy).not.toHaveBeenCalled()
    expect(result).toEqual({ continue: true })
  })

  it('does nothing for stages without gates', async () => {
    const pushSpy = vi.fn()
    const noGateWorkflow: WorkflowDefinition = {
      ...testWorkflow,
      nodes: [
        {
          id: 'stage-audit',
          type: 'workflowStage',
          position: { x: 0, y: 0 },
          data: { label: 'Audit', prompt: 'Do audit' },
        },
      ],
    }
    const gateHook = createGateHook(noGateWorkflow, pushSpy)

    const result = await gateHook(failInput, 'tool-1', {
      signal: new AbortController().signal,
    })

    expect(pushSpy).not.toHaveBeenCalled()
    expect(result).toEqual({ continue: true })
  })

  it('tracks retries across multiple invocations', async () => {
    const pushSpy = vi.fn()
    const gateHook = createGateHook(testWorkflow, pushSpy)

    // Attempt 1 — should retry (maxRetries = 3)
    await gateHook(failInput, 'tool-1', { signal: new AbortController().signal })
    expect(pushSpy.mock.calls[0][0].message.content[0].text).toContain('attempt 1/3')

    // Attempt 2 — should retry
    await gateHook(failInput, 'tool-2', { signal: new AbortController().signal })
    expect(pushSpy.mock.calls[1][0].message.content[0].text).toContain('attempt 2/3')

    // Attempt 3 — exhausted, should fail permanently
    await gateHook(failInput, 'tool-3', { signal: new AbortController().signal })
    expect(pushSpy.mock.calls[2][0].message.content[0].text).toContain('Workflow stage failed')
  })
})
