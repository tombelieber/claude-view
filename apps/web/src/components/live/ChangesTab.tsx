import type { FileHistoryResponse } from '../../types/generated/FileHistoryResponse'
import { FileChangeHeader } from './FileChangeHeader'

interface ChangesTabProps {
  fileHistory: FileHistoryResponse
  sessionId: string
}

export function ChangesTab({ fileHistory, sessionId }: ChangesTabProps) {
  const { summary, files } = fileHistory

  return (
    <div className="p-4 overflow-y-auto h-full space-y-2">
      {/* Summary header */}
      <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
        <span className="font-medium text-gray-700 dark:text-gray-300">
          {summary.totalFiles} file{summary.totalFiles !== 1 ? 's' : ''} changed
        </span>
        <span className="flex-1" />
        {summary.totalAdded > 0 && (
          <span className="font-mono text-green-600 dark:text-green-400">
            +{summary.totalAdded}
          </span>
        )}
        {summary.totalRemoved > 0 && (
          <span className="font-mono text-red-500 dark:text-red-400">−{summary.totalRemoved}</span>
        )}
      </div>

      {/* File list */}
      {files.map((file) => (
        <FileChangeHeader key={file.fileHash} file={file} sessionId={sessionId} />
      ))}
    </div>
  )
}
