// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const facetsGeneratedTools: ToolDef[] = [
  {
    name: 'facets_facet_badges',
    description: 'Quality badges (outcome + satisfaction) for the requested session IDs. Returns a JSON map keyed by session ID.',
    inputSchema: z.object({
    ids: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/facets/badges', { params: { ids: args.ids } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'facets_trigger_facet_ingest',
    description: 'Start facet ingest from the Claude Code insights cache. Returns immediately with `{"status": "started"}` or `{"status": "already_running"}` if an ingest is already in progress.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/facets/ingest/trigger')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'facets_pattern_alert',
    description: 'Check the most recent sessions for a negative satisfaction pattern. Returns `{pattern, count, tip}` if a pattern is detected, or `{pattern: null}` otherwise.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/facets/pattern-alert')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'facets_facet_stats',
    description: 'Aggregate statistics across all session facets.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/facets/stats')
      return JSON.stringify(result, null, 2)
    },
  }
]
