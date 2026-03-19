// sidecar/src/workflows/workflow-runner.ts
// Workflow orchestrator: creates a coordinator SDK session that dispatches
// stage agents via Agent Teams. Gate evaluation is deterministic (PostToolUse hook).

import type { HookInput, HookJSONOutput } from '@anthropic-ai/claude-agent-sdk'
import { query } from '@anthropic-ai/claude-agent-sdk'
import { findClaudeExecutable } from '../cli-path.js'
import { MessageBridge } from '../message-bridge.js'
import type { SessionRegistry } from '../session-registry.js'
import { buildAgentDefinitions } from './build-agents.js'
import { buildCoordinatorPrompt } from './coordinator-prompt.js'
import { extractAgentOutput } from './extract-agent-output.js'
import { evaluateGate } from './gate-evaluator.js'
import type { WorkflowDefinition } from './workflow-schema.js'

export interface WorkflowRunResponse {
  controlId: string
  sessionId: string
  teamName: string
}

/**
 * Creates a PostToolUse hook that evaluates gate conditions after Agent tool calls.
 * Tracks retry counts per stage and injects pass/fail/retry messages into the bridge.
 *
 * Exported for direct testing without SDK mocks.
 */
export function createGateHook(
  workflow: WorkflowDefinition,
  push: (msg: {
    type: 'user'
    message: { role: 'user'; content: Array<{ type: 'text'; text: string }> }
    parent_tool_use_id: null
    session_id: string
  }) => void,
) {
  const retryCount = new Map<string, number>()

  const makeUserMsg = (text: string) => ({
    type: 'user' as const,
    message: {
      role: 'user' as const,
      content: [{ type: 'text' as const, text }],
    },
    parent_tool_use_id: null,
    session_id: '',
  })

  return async (
    input: HookInput,
    _toolUseId: string | undefined,
    _options: { signal: AbortSignal },
  ): Promise<HookJSONOutput> => {
    if (input.tool_name !== 'Agent') return { continue: true }

    const toolInput = input.tool_input as { name?: string }
    const stageId = toolInput?.name
    if (!stageId) return { continue: true }

    const stage = workflow.nodes.find((n) => n.id === stageId)
    if (!stage || stage.type !== 'workflowStage' || !stage.data.gate) {
      return { continue: true }
    }

    const agentOutput = extractAgentOutput((input as { tool_response?: unknown }).tool_response)
    const passed = evaluateGate(stage.data.gate, agentOutput)
    const attempts = (retryCount.get(stageId) ?? 0) + 1

    if (!passed && attempts < stage.data.gate.maxRetries) {
      retryCount.set(stageId, attempts)
      push(
        makeUserMsg(
          `Gate FAILED for ${stageId} (attempt ${attempts}/${stage.data.gate.maxRetries}). Re-dispatch the same agent.`,
        ),
      )
    } else {
      // Either passed, or retries exhausted
      push(
        makeUserMsg(
          passed
            ? `Gate PASSED for ${stageId}. Proceed to next stage.`
            : `Gate FAILED for ${stageId} after ${attempts} retries. Workflow stage failed.`,
        ),
      )
    }

    return { continue: true }
  }
}

/**
 * Runs a workflow by creating a coordinator SDK session.
 * The coordinator dispatches stage agents in topological order,
 * and gate conditions are evaluated automatically via PostToolUse hooks.
 *
 * Returns immediately with control IDs -- the session runs in background.
 */
export async function runWorkflow(
  workflow: WorkflowDefinition,
  inputs: Record<string, string>,
  _registry: SessionRegistry,
): Promise<WorkflowRunResponse> {
  const teamName = `workflow-${workflow.id}-run-${Date.now()}`
  const coordinatorBridge = new MessageBridge()
  const agentDefs = buildAgentDefinitions(workflow, inputs)
  const systemPrompt = buildCoordinatorPrompt(workflow, inputs)

  const gateHook = createGateHook(workflow, (msg) => coordinatorBridge.push(msg))

  const controlId = `wf-${workflow.id}-${Date.now()}`

  // Start coordinator session (runs in background via async iteration)
  query({
    prompt: coordinatorBridge,
    options: {
      pathToClaudeCodeExecutable: findClaudeExecutable(),
      settingSources: (workflow.defaults.settingSources ?? ['user', 'project']) as Array<
        'user' | 'project'
      >,
      model: workflow.defaults.model,
      cwd: process.cwd(),
      effort: workflow.defaults.effort,
      maxBudgetUsd: workflow.defaults.maxBudgetUsd,
      maxTurns: workflow.defaults.maxTurns,
      permissionMode: (workflow.defaults.permissionMode ?? 'default') as 'default' | undefined,
      systemPrompt,
      agents: agentDefs,
      allowedTools: ['Agent', 'Read', 'Grep', 'Glob', 'TeamCreate', 'SendMessage'],
      hooks: {
        PostToolUse: [
          {
            matcher: 'Agent',
            hooks: [gateHook],
          },
        ],
      },
    },
  })

  return {
    controlId,
    sessionId: controlId,
    teamName,
  }
}
