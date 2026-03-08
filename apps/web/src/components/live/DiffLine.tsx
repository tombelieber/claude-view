import { cn } from '../../lib/utils'
import type { DiffLine as DiffLineType } from '../../types/generated/DiffLine'
import type { DiffLineKind } from '../../types/generated/DiffLineKind'

const LINE_BG: Record<DiffLineKind, string> = {
  context: '',
  add: 'bg-green-50 dark:bg-green-900/20',
  remove: 'bg-red-50 dark:bg-red-900/20',
}

const LINE_TEXT: Record<DiffLineKind, string> = {
  context: 'text-gray-700 dark:text-gray-300',
  add: 'text-green-800 dark:text-green-200',
  remove: 'text-red-800 dark:text-red-200',
}

const GUTTER_TEXT: Record<DiffLineKind, string> = {
  context: 'text-gray-400 dark:text-gray-600',
  add: 'text-green-600 dark:text-green-500',
  remove: 'text-red-500 dark:text-red-400',
}

const PREFIX: Record<DiffLineKind, string> = {
  context: ' ',
  add: '+',
  remove: '−',
}

interface DiffLineProps {
  line: DiffLineType
}

export function DiffLineRow({ line }: DiffLineProps) {
  return (
    <div className={cn('flex', LINE_BG[line.kind])}>
      <span
        className={cn(
          'w-10 text-right pr-2 text-[10px] font-mono select-none border-r border-gray-200 dark:border-gray-800 shrink-0',
          GUTTER_TEXT[line.kind],
        )}
      >
        {line.oldLineNo ?? ''}
      </span>
      <span
        className={cn(
          'w-10 text-right pr-2 text-[10px] font-mono select-none border-r border-gray-200 dark:border-gray-800 shrink-0',
          GUTTER_TEXT[line.kind],
        )}
      >
        {line.newLineNo ?? ''}
      </span>
      <span
        className={cn(
          'w-4 text-center text-[11px] font-mono select-none shrink-0',
          GUTTER_TEXT[line.kind],
        )}
      >
        {PREFIX[line.kind]}
      </span>
      <span className={cn('flex-1 pl-1 text-xs font-mono whitespace-pre', LINE_TEXT[line.kind])}>
        {line.content}
      </span>
    </div>
  )
}
