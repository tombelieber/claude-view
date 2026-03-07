import { cpSync, existsSync, mkdirSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const pluginRoot = resolve(__dirname, '..')
const mcpDist = resolve(pluginRoot, '..', 'mcp', 'dist')
const pluginDist = resolve(pluginRoot, 'dist')

// Guard: ensure packages/mcp was built first
if (!existsSync(mcpDist)) {
  console.error(`ERROR: packages/mcp/dist not found at ${mcpDist}`)
  console.error('Run: bun run build --filter=@claude-view/mcp first')
  process.exit(1)
}

mkdirSync(pluginDist, { recursive: true })

// Copy entire MCP dist — preserves internal imports (server.js, client.js, tools/)
cpSync(mcpDist, pluginDist, { recursive: true })

console.log(`Bundled MCP server → ${pluginDist}/`)
