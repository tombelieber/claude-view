import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js'
import { ClaudeViewClient } from './client.js'
import { liveTools } from './tools/live.js'
import { sessionTools } from './tools/sessions.js'
import { statsTools } from './tools/stats.js'

const ALL_TOOLS = [...sessionTools, ...statsTools, ...liveTools]
export const TOOL_COUNT = ALL_TOOLS.length

export function createServer(port?: number) {
  const client = new ClaudeViewClient(port)

  const server = new McpServer({
    name: 'claude-view',
    version: '0.8.0',
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
