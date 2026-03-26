// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const ideGeneratedTools: ToolDef[] = [
  {
    name: 'ide_get_detect',
    description: '`GET /api/ide/detect` — return cached list of installed IDEs.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/ide/detect')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'ide_post_open',
    description: '`POST /api/ide/open` — open a file in the requested IDE.',
    inputSchema: z.object({
    filePath: z.string().describe('Relative path to the file within the project (optional).').optional(),
    ide: z.string().describe('The IDE id to open (must match an id from the detect response).'),
    projectPath: z.string().describe('Absolute path to the project directory.'),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/ide/open', { body: { filePath: args.filePath, ide: args.ide, projectPath: args.projectPath } })
      return JSON.stringify(result, null, 2)
    },
  }
]
