import { useQuery } from '@tanstack/react-query'

interface HealthResponse {
  status: string
  version: string
  uptime_secs: number
}

export function HealthIndicator() {
  const { data, isError, isLoading } = useQuery<HealthResponse>({
    queryKey: ['health'],
    queryFn: async () => {
      const response = await fetch('/api/health')
      if (!response.ok) throw new Error('Health check failed')
      return response.json()
    },
    refetchInterval: 30000, // Poll every 30 seconds
    retry: 1,
    staleTime: 10000,
  })

  if (isLoading) {
    return (
      <span
        className="inline-block w-2 h-2 rounded-full bg-yellow-400 animate-pulse"
        title="Checking backend status..."
      />
    )
  }

  if (isError) {
    return (
      <span
        className="inline-block w-2 h-2 rounded-full bg-red-500"
        title="Backend offline"
      />
    )
  }

  if (data?.status === 'ok') {
    return (
      <span
        className="inline-block w-2 h-2 rounded-full bg-green-500"
        title={`Backend online (v${data.version})`}
      />
    )
  }

  return (
    <span
      className="inline-block w-2 h-2 rounded-full bg-yellow-400"
      title="Unknown status"
    />
  )
}
