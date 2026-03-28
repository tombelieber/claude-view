import { createRequire } from 'node:module'
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js'
import { ClaudeViewClient } from './client.js'
import { allGeneratedTools } from './tools/generated/index.js'
import { liveTools } from './tools/live.js'
import { sessionTools } from './tools/sessions.js'
import { statsTools } from './tools/stats.js'

const require = createRequire(import.meta.url)
const { version } = require('../package.json') as { version: string }

// Hand-written tools take precedence — dedup by name
const HAND_WRITTEN = [...sessionTools, ...statsTools, ...liveTools]
const handWrittenNames = new Set(HAND_WRITTEN.map(t => t.name))
const dedupedGenerated = allGeneratedTools.filter(t => !handWrittenNames.has(t.name))
const ALL_TOOLS = [...HAND_WRITTEN, ...dedupedGenerated]
export const TOOL_COUNT = ALL_TOOLS.length

export function createServer(port?: number) {
  const client = new ClaudeViewClient(port)

  const server = new McpServer({
    name: 'claude-view',
    version,
  })

  for (const tool of ALL_TOOLS) {
    server.registerTool(
      tool.name,
      {
        description: tool.description,
        inputSchema: tool.inputSchema,
        annotations: tool.annotations,
      },
      async (args: any) => {
        try {
          const result = await tool.handler(client, args)
          return { content: [{ type: 'text' as const, text: result }] }
        } catch (err: any) {
          return {
            content: [{ type: 'text' as const, text: `Error: ${err.message}` }],
            isError: true,
          }
        }
      },
    )
  }

  return server
}
