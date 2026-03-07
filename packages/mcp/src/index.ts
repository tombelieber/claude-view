#!/usr/bin/env node

import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import { TOOL_COUNT, createServer } from './server.js'

const port = process.env.CLAUDE_VIEW_PORT ? Number(process.env.CLAUDE_VIEW_PORT) : undefined
const server = createServer(port)
const transport = new StdioServerTransport()

console.error(`claude-view MCP server starting (${TOOL_COUNT} tools, port ${port ?? 47892})`)

await server.connect(transport)
