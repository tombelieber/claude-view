import { interpolatePrompt } from './interpolate.js'
import type { WorkflowDefinition } from './workflow-schema.js'

/** Agent definition shape -- inline type to avoid SDK import dependency */
export interface AgentDefinition {
  description: string
  prompt: string
  model?: string
  tools?: string[]
  disallowedTools?: string[]
  skills?: string[]
  maxTurns?: number
}

/**
 * Converts workflowStage nodes into a Record of AgentDefinitions,
 * keyed by node ID. Skips input/done nodes. Interpolates prompt
 * templates with the provided input values.
 */
export function buildAgentDefinitions(
  workflow: WorkflowDefinition,
  inputValues: Record<string, string>,
): Record<string, AgentDefinition> {
  const agents: Record<string, AgentDefinition> = {}

  for (const node of workflow.nodes) {
    if (node.type !== 'workflowStage') continue

    const d = node.data
    agents[node.id] = {
      description: `Workflow stage: ${d.label}. ${d.prompt.slice(0, 100)}...`,
      prompt: interpolatePrompt(d.prompt, workflow.inputs, inputValues),
      model: d.model,
      tools: d.tools,
      disallowedTools: d.disallowedTools,
      skills: d.skills,
      maxTurns: d.maxTurns,
    }
  }

  return agents
}
