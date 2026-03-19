import { topologicalSort } from './dag-sort.js'
import { interpolatePrompt } from './interpolate.js'
import type { WorkflowDefinition } from './workflow-schema.js'

/**
 * Builds the system prompt for the workflow coordinator agent.
 * Validates that at least one stage exists and that all referenced
 * input variables have values. Returns a prompt with topologically
 * sorted execution order and dispatch instructions.
 */
export function buildCoordinatorPrompt(
  workflow: WorkflowDefinition,
  inputs: Record<string, string>,
): string {
  const stageNodes = workflow.nodes.filter((n) => n.type === 'workflowStage')
  if (stageNodes.length === 0) throw new Error('Workflow has no stage nodes')

  // Validate all template variables in stage prompts are satisfied
  for (const node of stageNodes) {
    if (node.type !== 'workflowStage') continue
    interpolatePrompt(node.data.prompt, workflow.inputs, inputs)
  }

  // Topological sort on stage nodes only
  const stageIds = stageNodes.map((n) => n.id)
  const stageEdges = workflow.edges
    .filter((e) => stageIds.includes(e.source) && stageIds.includes(e.target))
    .filter((e) => e.source !== e.target)

  const stageOrder = topologicalSort(stageIds, stageEdges)

  return `You are a workflow orchestrator. Execute this workflow step by step.

WORKFLOW: ${workflow.name}
INPUTS: ${JSON.stringify(inputs)}

EXECUTION ORDER (topologically sorted):
${stageOrder.map((id, i) => `${i + 1}. ${id}`).join('\n')}

INSTRUCTIONS:
1. Create a team named "workflow-${workflow.id}-run-${Date.now()}"
2. For each stage in order, use the Agent tool to dispatch the named agent
   (pass team_name and name parameters to Agent tool)
3. Wait for each agent to complete before proceeding
4. After each agent completes, you will receive a message indicating gate pass/fail
5. If a gate fails with retries remaining, re-dispatch the same agent
6. When all stages complete, summarize the workflow run

IMPORTANT: Gate conditions are evaluated by the system automatically.
You will receive gate results as messages after each agent completes.
Just follow the pass/fail/retry instructions you receive.`
}
