// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const mcpGeneratedTools: ToolDef[] = [
  {
    name: 'mcp_get_mcp_servers',
    description: 'returns all deduplicated MCP server configurations.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/mcp-servers')
      return JSON.stringify(result, null, 2)
    },
  },
]
