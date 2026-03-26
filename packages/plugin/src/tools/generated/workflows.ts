// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const workflowsGeneratedTools: ToolDef[] = [
  {
    name: 'workflows_list_workflows',
    description: 'GET /api/workflows',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/workflows')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'workflows_create_workflow',
    description: 'POST /api/workflows',
    inputSchema: z.object({
    yaml: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/workflows', { body: { yaml: args.yaml } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'workflows_control_run',
    description: 'POST /api/workflows/run/{run_id}/control',
    inputSchema: z.object({
    run_id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', `/api/workflows/run/${args.run_id}/control`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'workflows_get_workflow',
    description: 'GET /api/workflows/{id}',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/workflows/${args.id}`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'workflows_delete_workflow',
    description: 'DELETE /api/workflows/{id}',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('DELETE', `/api/workflows/${args.id}`)
      return JSON.stringify(result, null, 2)
    },
  }
]
