// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const teamsGeneratedTools: ToolDef[] = [
  {
    name: 'teams_list_teams',
    description: 'List all teams.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/teams')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'teams_get_team',
    description: 'Get team detail.',
    inputSchema: z.object({
    name: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/teams/${args.name}`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'teams_get_team_inbox',
    description: 'Get team inbox messages.',
    inputSchema: z.object({
    name: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/teams/${args.name}/inbox`)
      return JSON.stringify(result, null, 2)
    },
  }
]
