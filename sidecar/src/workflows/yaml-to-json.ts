import type { WorkflowDefinition } from './workflow-schema.js'

// --- Legacy YAML types ---

interface LegacyGate {
  condition?: string
  retry?: boolean
  maxAttempts?: number
}

interface LegacyStage {
  id?: string
  name: string
  prompt?: string
  skills?: string[]
  model?: string
  tools?: string[]
  gate?: LegacyGate
}

interface LegacyWorkflow {
  name: string
  description: string
  author: string
  category: string
  version: string
  inputs: Array<{ name: string; type: string; description?: string }>
  stages: LegacyStage[]
}

// --- Converter ---

export function convertYamlToJson(legacy: LegacyWorkflow): WorkflowDefinition {
  const id = legacy.name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')

  const stages = legacy.stages ?? []

  const inputNodes = buildInputNodes(legacy.inputs)
  const stageNodes = buildStageNodes(stages)
  const doneNode = buildDoneNode(stages.length)
  const edges = buildEdges(inputNodes, stageNodes)

  return {
    id,
    name: legacy.name,
    description: legacy.description,
    author: legacy.author,
    version: legacy.version,
    category: legacy.category,
    inputs: legacy.inputs.map((inp) => ({
      name: inp.name,
      type: inp.type as 'file' | 'string',
      ...(inp.description ? { description: inp.description } : {}),
    })),
    defaults: { model: 'claude-sonnet-4-6' },
    nodes: [...inputNodes, ...stageNodes, doneNode],
    edges,
  }
}

// --- Internal helpers ---

function buildInputNodes(inputs: LegacyWorkflow['inputs']) {
  return inputs.map((inp, i) => ({
    id: `input-${i}`,
    type: 'workflowInput' as const,
    position: { x: 0, y: i * 100 },
    data: { label: inp.name },
  }))
}

function buildStageNodes(stages: LegacyStage[]) {
  return stages.map((stage, i) => {
    const stageId = stage.id ?? `stage-${stage.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`
    const gate = stage.gate ? convertLegacyGate(stage.gate) : undefined

    return {
      id: stageId,
      type: 'workflowStage' as const,
      position: { x: (i + 1) * 300, y: 0 },
      data: {
        label: stage.name,
        prompt: stage.prompt ?? `Run ${stage.name}`,
        ...(stage.model ? { model: stage.model } : {}),
        ...(stage.tools ? { tools: stage.tools } : {}),
        ...(stage.skills ? { skills: stage.skills } : {}),
        ...(gate ? { gate } : {}),
      },
    }
  })
}

function buildDoneNode(stageCount: number) {
  return {
    id: 'done-0',
    type: 'workflowDone' as const,
    position: { x: (stageCount + 1) * 300, y: 0 },
    data: { label: 'Done' },
  }
}

function buildEdges(
  inputNodes: ReturnType<typeof buildInputNodes>,
  stageNodes: ReturnType<typeof buildStageNodes>,
) {
  const edges: WorkflowDefinition['edges'] = []
  const stageIds = stageNodes.map((n) => n.id)

  // input -> first stage
  if (inputNodes.length > 0 && stageIds.length > 0) {
    edges.push({
      id: `e-input-${stageIds[0]}`,
      source: inputNodes[0].id,
      target: stageIds[0],
    })
  }

  // stage -> stage chain
  for (let i = 0; i < stageIds.length - 1; i++) {
    edges.push({
      id: `e-${stageIds[i]}-${stageIds[i + 1]}`,
      source: stageIds[i],
      target: stageIds[i + 1],
    })
  }

  // last stage -> done
  if (stageIds.length > 0) {
    edges.push({
      id: `e-${stageIds[stageIds.length - 1]}-done`,
      source: stageIds[stageIds.length - 1],
      target: 'done-0',
    })
  } else if (inputNodes.length > 0) {
    edges.push({
      id: 'e-input-done',
      source: inputNodes[0].id,
      target: 'done-0',
    })
  }

  return edges
}

function convertLegacyGate(legacy: LegacyGate) {
  return {
    type: 'regex' as const,
    pattern: legacy.condition?.replace(/\s*==\s*/g, '') ?? '.*',
    maxRetries: legacy.retry ? (legacy.maxAttempts ?? 3) : 0,
  }
}
