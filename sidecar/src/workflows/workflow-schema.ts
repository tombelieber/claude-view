import { z } from 'zod'

// --- Shared primitives ---

const positionSchema = z.object({ x: z.number(), y: z.number() })

// --- Gate conditions (discriminated union on 'type') ---

const gateConditionSchema = z.discriminatedUnion('type', [
  z.object({
    type: z.literal('json_field'),
    field: z.string(),
    operator: z.enum(['equals', 'contains', 'gt', 'lt']),
    value: z.union([z.string(), z.number()]),
    maxRetries: z.number().int().min(0),
  }),
  z.object({
    type: z.literal('regex'),
    pattern: z.string(),
    maxRetries: z.number().int().min(0),
  }),
  z.object({
    type: z.literal('exit_code'),
    value: z.union([z.string(), z.number()]),
    maxRetries: z.number().int().min(0),
  }),
  z.object({
    type: z.literal('file_exists'),
    path: z.string(),
    maxRetries: z.number().int().min(0),
  }),
])

export type GateCondition = z.infer<typeof gateConditionSchema>

// --- Node schemas (discriminated union on 'type') ---

const workflowInputNodeSchema = z.object({
  id: z.string(),
  type: z.literal('workflowInput'),
  position: positionSchema,
  data: z.object({ label: z.string() }),
})

const workflowStageNodeSchema = z.object({
  id: z.string(),
  type: z.literal('workflowStage'),
  position: positionSchema,
  data: z.object({
    label: z.string(),
    prompt: z.string(),
    model: z.string().optional(),
    tools: z.array(z.string()).optional(),
    skills: z.array(z.string()).optional(),
    maxTurns: z.number().optional(),
    disallowedTools: z.array(z.string()).optional(),
    gate: gateConditionSchema.optional(),
  }),
})

const workflowDoneNodeSchema = z.object({
  id: z.string(),
  type: z.literal('workflowDone'),
  position: positionSchema,
  data: z.object({ label: z.string() }),
})

const workflowNodeSchema = z.discriminatedUnion('type', [
  workflowInputNodeSchema,
  workflowStageNodeSchema,
  workflowDoneNodeSchema,
])

export type WorkflowNode = z.infer<typeof workflowNodeSchema>

// --- Edge schema ---

const workflowEdgeSchema = z.object({
  id: z.string(),
  source: z.string(),
  target: z.string(),
  label: z.string().optional(),
  data: z.record(z.string(), z.unknown()).optional(),
})

// --- Workflow-level schemas ---

const workflowDefaultsSchema = z.object({
  model: z.string(),
  permissionMode: z.string().optional(),
  settingSources: z.array(z.string()).optional(),
  effort: z.enum(['low', 'medium', 'high', 'max']).optional(),
  maxBudgetUsd: z.number().optional(),
  maxTurns: z.number().optional(),
})

const workflowInputFieldSchema = z.object({
  name: z.string(),
  type: z.enum(['file', 'string']),
  description: z.string().optional(),
})

// --- Top-level workflow schema with edge referential integrity ---

export const workflowSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    description: z.string(),
    author: z.string(),
    version: z.string(),
    category: z.string(),
    inputs: z.array(workflowInputFieldSchema),
    defaults: workflowDefaultsSchema,
    nodes: z.array(workflowNodeSchema),
    edges: z.array(workflowEdgeSchema),
  })
  .superRefine((data, ctx) => {
    const nodeIds = new Set(data.nodes.map((n) => n.id))
    for (const edge of data.edges) {
      if (!nodeIds.has(edge.source)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          message: `Edge ${edge.id}: source '${edge.source}' not found`,
          path: ['edges'],
        })
      }
      if (!nodeIds.has(edge.target)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          message: `Edge ${edge.id}: target '${edge.target}' not found`,
          path: ['edges'],
        })
      }
    }
  })

export type WorkflowDefinition = z.infer<typeof workflowSchema>
