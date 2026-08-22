// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const claudehomeGeneratedTools: ToolDef[] = [
  {
    name: 'claude_home_list_claude_home',
    description: 'List Claude Home (GET /api/claude-home)',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/claude-home')
      return JSON.stringify(result, null, 2)
    },
  },
]
