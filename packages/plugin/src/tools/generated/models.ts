// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const modelsGeneratedTools: ToolDef[] = [
  {
    name: 'models_list_models',
    description: 'List all known models with usage counts.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/models')
      return JSON.stringify(result, null, 2)
    },
  }
]
