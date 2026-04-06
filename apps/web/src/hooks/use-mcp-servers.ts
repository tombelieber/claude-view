import { useQuery } from '@tanstack/react-query'

// ── Types matching Rust McpServer / McpServerIndex ──

export interface McpServer {
  name: string
  serverType: string | null
  url: string | null
  hasOauth: boolean
  needsReauth: boolean
}

export interface McpServerIndex {
  servers: McpServer[]
  rawFileCount: number
}

// ── Fetch ──

async function fetchMcpServers(): Promise<McpServerIndex> {
  const res = await fetch('/api/mcp-servers')
  if (!res.ok) throw new Error('Failed to fetch MCP servers')
  return res.json()
}

// ── Hook ──

/** Fetch deduplicated MCP server configurations from plugin cache. */
export function useMcpServers() {
  return useQuery({
    queryKey: ['mcp-servers'],
    queryFn: fetchMcpServers,
    staleTime: 60_000, // configs don't change frequently
  })
}
