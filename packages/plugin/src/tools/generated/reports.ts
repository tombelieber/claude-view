// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const reportsGeneratedTools: ToolDef[] = [
  {
    name: 'reports_list_reports',
    description: 'List all saved reports.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/reports')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'reports_get_preview',
    description: 'Aggregate preview stats for a date range.',
    inputSchema: z.object({
    startTs: z.number().optional(),
    endTs: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/reports/preview', { params: { startTs: args.startTs, endTs: args.endTs } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'reports_get_report',
    description: 'Get a single report.',
    inputSchema: z.object({
    id: z.number(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/reports/${encodeURIComponent(String(args.id))}`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'reports_delete_report',
    description: 'Delete a report.',
    inputSchema: z.object({
    id: z.number(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('DELETE', `/api/reports/${encodeURIComponent(String(args.id))}`)
      return JSON.stringify(result, null, 2)
    },
  }
]
