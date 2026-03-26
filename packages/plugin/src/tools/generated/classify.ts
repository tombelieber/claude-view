// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const classifyGeneratedTools: ToolDef[] = [
  {
    name: 'classify_start_classification',
    description: 'Trigger a classification job.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/classify')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'classify_cancel_classification',
    description: 'Cancel a running classification job.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/classify/cancel')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'classify_single_session',
    description: 'Classify a single session synchronously.',
    inputSchema: z.object({
    session_id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', `/api/classify/single/${encodeURIComponent(String(args.session_id))}`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'classify_get_classification_status',
    description: 'Get classification status.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/classify/status')
      return JSON.stringify(result, null, 2)
    },
  }
]
