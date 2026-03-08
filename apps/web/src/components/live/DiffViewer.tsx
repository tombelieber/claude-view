import type { DiffHunk } from '../../types/generated/DiffHunk'
import { DiffLineRow } from './DiffLine'

interface DiffViewerProps {
  hunks: DiffHunk[]
}

export function DiffViewer({ hunks }: DiffViewerProps) {
  return (
    <div className="text-xs font-mono overflow-x-auto">
      {hunks.map((hunk, hunkIdx) => (
        <div key={`${hunk.oldStart}-${hunk.newStart}`}>
          {hunkIdx > 0 && (
            <div className="bg-gray-50 dark:bg-gray-800/50 text-[10px] text-gray-400 dark:text-gray-500 italic text-center py-1 border-y border-gray-200 dark:border-gray-800">
              ···
            </div>
          )}
          {hunk.lines.map((line) => (
            <DiffLineRow
              key={`${line.oldLineNo ?? 'n'}-${line.newLineNo ?? 'n'}-${line.kind}`}
              line={line}
            />
          ))}
        </div>
      ))}
    </div>
  )
}
