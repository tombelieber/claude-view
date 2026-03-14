import { useCallback, useEffect, useRef, useState } from 'react'

interface WsQueryResult<T> {
  data: T | null
  loading: boolean
  error: Error | null
  refresh: () => void
}

export function useWsQuery<T>(
  queryFn: (() => Promise<T>) | null,
  options?: { autoFetch?: boolean },
): WsQueryResult<T> {
  const willAutoFetch = options?.autoFetch !== false && queryFn !== null
  const [data, setData] = useState<T | null>(null)
  const [loading, setLoading] = useState(willAutoFetch)
  const [error, setError] = useState<Error | null>(null)
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const refresh = useCallback(() => {
    if (!queryFn) return
    setLoading(true)
    setError(null)
    queryFn()
      .then((result) => {
        if (mountedRef.current) {
          setData(result)
          setLoading(false)
        }
      })
      .catch((err) => {
        if (mountedRef.current) {
          setError(err instanceof Error ? err : new Error(String(err)))
          setLoading(false)
        }
      })
  }, [queryFn])

  useEffect(() => {
    if (options?.autoFetch !== false && queryFn) {
      refresh()
    }
  }, [refresh, options?.autoFetch, queryFn])

  return { data, loading, error, refresh }
}
