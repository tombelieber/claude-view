// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const cliGeneratedTools: ToolDef[] = [
  {
    name: 'cli_create_session',
    description: '- Create a new tmux-backed CLI session.',
    inputSchema: z.object({
      args: z.array(z.unknown()).describe('Additional CLI args to pass to `claude`.').optional(),
      projectDir: z.string().describe('Optional working directory for the CLI session.').optional(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/cli-sessions', {
        body: { args: args.args, projectDir: args.projectDir },
      })
      return JSON.stringify(result, null, 2)
    },
  },
]
