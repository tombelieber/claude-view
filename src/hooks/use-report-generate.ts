import { useState, useCallback, useRef } from 'react'
import { useReportsMutate } from './use-reports'

interface GenerateParams {
  reportType: string
  dateStart: string
  dateEnd: string
  startTs: number
  endTs: number
}

interface UseReportGenerateReturn {
  generate: (params: GenerateParams) => void
  isGenerating: boolean
  streamedText: string
  error: string | null
  reset: () => void
}

export function useReportGenerate(): UseReportGenerateReturn {
  const [isGenerating, setIsGenerating] = useState(false)
  const [streamedText, setStreamedText] = useState('')
  const [error, setError] = useState<string | null>(null)
  const abortRef = useRef<AbortController | null>(null)
  const invalidateReports = useReportsMutate()

  const reset = useCallback(() => {
    setStreamedText('')
    setError(null)
    setIsGenerating(false)
  }, [])

  const generate = useCallback(async (params: GenerateParams) => {
    reset()
    setIsGenerating(true)

    const controller = new AbortController()
    abortRef.current = controller

    try {
      const res = await fetch('/api/reports/generate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(params),
        signal: controller.signal,
      })

      if (!res.ok) {
        const text = await res.text()
        throw new Error(text || `HTTP ${res.status}`)
      }

      const reader = res.body?.getReader()
      if (!reader) throw new Error('No response body')

      const decoder = new TextDecoder()
      let buffer = ''
      let accumulated = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })

        // Parse SSE events from buffer
        const lines = buffer.split('\n')
        buffer = lines.pop() || '' // Keep incomplete line in buffer

        let eventType = ''
        for (const line of lines) {
          if (line.startsWith('event: ')) {
            eventType = line.slice(7).trim()
          } else if (line.startsWith('data: ')) {
            const data = line.slice(6)
            try {
              const parsed = JSON.parse(data)

              if (eventType === 'chunk') {
                if (accumulated) accumulated += '\n'
                accumulated += parsed.text
                setStreamedText(accumulated)
              } else if (eventType === 'done') {
                // Report was saved -- fetch the full report
                invalidateReports()
                // We don't have the full ReportRow from the SSE, but we know it's done
                setIsGenerating(false)
              } else if (eventType === 'error') {
                setError(parsed.message || 'Generation failed')
                setIsGenerating(false)
              }
            } catch {
              // Skip unparseable lines
            }
          }
        }
      }

      setIsGenerating(false)
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') return
      setError(err instanceof Error ? err.message : 'Unknown error')
      setIsGenerating(false)
    }
  }, [reset, invalidateReports])

  return { generate, isGenerating, streamedText, error, reset }
}
