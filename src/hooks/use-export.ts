import { useState, useCallback } from 'react'
import type { ExportResponse } from '../types/generated'

export type ExportFormat = 'json' | 'csv'

interface UseExportResult {
  /** Export sessions and trigger browser download */
  exportSessions: (format?: ExportFormat) => Promise<void>
  /** Whether an export is currently in progress */
  isExporting: boolean
  /** Error message if export failed */
  error: string | null
  /** Clear the error state */
  clearError: () => void
}

/**
 * Generate a filename with timestamp for export.
 */
function generateFilename(format: ExportFormat): string {
  const date = new Date().toISOString().split('T')[0]
  return `sessions-${date}.${format}`
}

/**
 * Trigger browser download of a blob.
 */
function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

/**
 * Hook for exporting session data.
 *
 * Supports JSON and CSV formats:
 * - JSON: Returns ExportResponse with sessions array
 * - CSV: Returns RFC 4180 compliant CSV text
 *
 * Both formats trigger a browser download.
 */
export function useExport(): UseExportResult {
  const [isExporting, setIsExporting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const clearError = useCallback(() => {
    setError(null)
  }, [])

  const exportSessions = useCallback(async (format: ExportFormat = 'json') => {
    setIsExporting(true)
    setError(null)

    try {
      const response = await fetch(`/api/export/sessions?format=${format}`)

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ details: 'Unknown error' }))
        throw new Error(errorData.details || `Export failed with status ${response.status}`)
      }

      if (format === 'csv') {
        // CSV: Get text and create blob
        const text = await response.text()
        const blob = new Blob([text], { type: 'text/csv; charset=utf-8' })
        downloadBlob(blob, generateFilename('csv'))
      } else {
        // JSON: Get JSON and create formatted blob
        const data: ExportResponse = await response.json()
        const jsonStr = JSON.stringify(data, null, 2)
        const blob = new Blob([jsonStr], { type: 'application/json; charset=utf-8' })
        downloadBlob(blob, generateFilename('json'))
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Export failed'
      setError(message)
      throw err
    } finally {
      setIsExporting(false)
    }
  }, [])

  return {
    exportSessions,
    isExporting,
    error,
    clearError,
  }
}

// Re-export types for convenience
export type { ExportResponse, ExportedSession } from '../types/generated'
