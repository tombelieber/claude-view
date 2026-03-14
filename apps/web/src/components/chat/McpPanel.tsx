import { useCallback, useRef, useState } from 'react'

interface McpServer {
  name: string
  status: string
}

interface McpPanelProps {
  queryMcpStatus: () => Promise<unknown>
  toggleMcp: (serverName: string, enabled: boolean) => void
  reconnectMcp: (serverName: string) => void
}

export function McpPanel({ queryMcpStatus, toggleMcp, reconnectMcp }: McpPanelProps) {
  const [servers, setServers] = useState<McpServer[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const fetchStatus = useCallback(() => {
    setLoading(true)
    queryMcpStatus()
      .then((data) => {
        setServers(data as McpServer[])
        setLoading(false)
        setError(null)
      })
      .catch((err) => {
        setError(err instanceof Error ? err : new Error(String(err)))
        setLoading(false)
      })
  }, [queryMcpStatus])

  // Auto-fetch on mount
  useState(() => {
    fetchStatus()
  })

  const debouncedRefresh = useCallback(() => {
    if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current)
    refreshTimerRef.current = setTimeout(() => {
      fetchStatus()
    }, 500)
  }, [fetchStatus])

  const handleToggle = useCallback(
    (name: string, currentlyConnected: boolean) => {
      toggleMcp(name, !currentlyConnected)
      debouncedRefresh()
    },
    [toggleMcp, debouncedRefresh],
  )

  const handleReconnect = useCallback(
    (name: string) => {
      reconnectMcp(name)
      debouncedRefresh()
    },
    [reconnectMcp, debouncedRefresh],
  )

  if (loading && servers.length === 0) {
    return <div className="p-4 text-text-secondary text-sm">Loading MCP servers...</div>
  }

  if (error) {
    return <div className="p-4 text-text-error text-sm">Failed to load MCP status</div>
  }

  if (servers.length === 0) {
    return <div className="p-4 text-text-secondary text-sm">No MCP servers configured</div>
  }

  return (
    <div className="p-4 space-y-2">
      <h3 className="font-medium text-sm">MCP Servers</h3>
      <div className="space-y-1">
        {servers.map((server) => {
          const isConnected = server.status === 'connected'
          return (
            <div
              key={server.name}
              className="flex items-center justify-between px-3 py-2 rounded bg-bg-secondary text-sm"
            >
              <div className="flex items-center gap-2">
                <span
                  className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`}
                />
                <span>{server.name}</span>
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => handleToggle(server.name, isConnected)}
                  className="text-xs px-2 py-0.5 rounded border border-border-secondary hover:bg-bg-tertiary"
                  title={isConnected ? 'Disable' : 'Enable'}
                >
                  {isConnected ? 'Disable' : 'Enable'}
                </button>
                <button
                  type="button"
                  onClick={() => handleReconnect(server.name)}
                  className="text-xs px-2 py-0.5 rounded border border-border-secondary hover:bg-bg-tertiary"
                  title="Reconnect"
                >
                  Reconnect
                </button>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
