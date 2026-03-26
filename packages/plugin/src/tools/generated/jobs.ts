// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const jobsGeneratedTools: ToolDef[] = [
  {
    name: 'jobs_list_jobs',
    description: 'List all active jobs.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/jobs')
      return JSON.stringify(result, null, 2)
    },
  }
]
