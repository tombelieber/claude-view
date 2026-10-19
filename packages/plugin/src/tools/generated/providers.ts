// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const providersGeneratedTools: ToolDef[] = [
  {
    name: 'providers_list_providers',
    description: 'providers with session counts (count > 0 only, Claude Code always first).',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/providers')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'providers_usage',
    description:
      "token/cost aggregates per foreign provider within a trailing window. Claude Code is NOT included: CC usage comes from the existing rollup pipeline; this endpoint reads the foreign catalog's cached metadata (no files parsed beyond the cache fill).",
    inputSchema: z.object({
      days: z.number().optional(),
    }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/providers/usage', {
        params: { days: args.days },
      })
      return JSON.stringify(result, null, 2)
    },
  },
]
