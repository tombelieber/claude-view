import { useWsQuery } from '../../hooks/use-ws-query'

interface AccountInfoPanelProps {
  queryAccountInfo: () => Promise<unknown>
}

export function AccountInfoPanel({ queryAccountInfo }: AccountInfoPanelProps) {
  const { data, loading, error } = useWsQuery(queryAccountInfo)

  if (loading) return <div className="p-4 text-text-secondary">Loading...</div>
  if (error) return <div className="p-4 text-text-error">Failed to load account info</div>
  if (!data) return null

  return (
    <div className="p-4 space-y-2 text-sm">
      <h3 className="font-medium">Account Info</h3>
      <pre className="text-xs bg-bg-secondary p-2 rounded overflow-auto max-h-48">
        {JSON.stringify(data, null, 2)}
      </pre>
    </div>
  )
}
