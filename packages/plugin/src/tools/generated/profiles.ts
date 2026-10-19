// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const profilesGeneratedTools: ToolDef[] = [
  {
    name: 'profiles_list_profiles',
    description: 'List Profiles (GET /api/profiles)',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/profiles')
      return JSON.stringify(result, null, 2)
    },
  },
]
