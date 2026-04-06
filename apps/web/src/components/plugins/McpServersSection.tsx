import { Globe, Loader2 } from 'lucide-react'
import { useMcpServers, type McpServer } from '../../hooks/use-mcp-servers'

function McpServerRow({ server }: { server: McpServer }) {
  return (
    <div className="flex items-center gap-3 px-3 py-2 border-b border-gray-100 dark:border-gray-800/50 last:border-b-0">
      <span className="text-sm font-medium text-gray-900 dark:text-gray-100 w-32 truncate">
        {server.name}
      </span>
      <span className="text-xs font-mono text-gray-400 dark:text-gray-500 w-12">
        {server.serverType ?? '—'}
      </span>
      <span className="text-xs font-mono text-gray-500 dark:text-gray-400 flex-1 truncate">
        {server.url ?? '(no URL configured)'}
      </span>
      {server.hasOauth && (
        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs font-medium bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300">
          🔐 OAuth
        </span>
      )}
      {server.needsReauth && (
        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300">
          ⚠ needs auth
        </span>
      )}
    </div>
  )
}

export function McpServersSection() {
  const { data, isLoading } = useMcpServers()

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-xs text-apple-text3 py-3">
        <Loader2 className="w-3.5 h-3.5 animate-spin" />
        Loading MCP servers…
      </div>
    )
  }

  if (!data || data.servers.length === 0) return null

  return (
    <section>
      <div className="flex items-center gap-2 mb-2">
        <Globe className="w-4 h-4 text-apple-text3" />
        <h2 className="text-sm font-semibold text-apple-text1 tracking-tight">MCP Servers</h2>
        <span className="text-xs text-apple-text3">({data.servers.length})</span>
      </div>
      <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900/50 overflow-hidden">
        {data.servers.map((server) => (
          <McpServerRow key={server.name} server={server} />
        ))}
      </div>
    </section>
  )
}
