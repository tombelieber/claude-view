import { useState } from 'react'
import { cn } from '../../lib/utils'

// ── Types ──────────────────────────────────────────────────────────────────

interface TaskAssignment {
  type: 'task_assignment'
  taskId: string
  subject: string
  description?: string
  assignedBy?: string
  timestamp?: string
}

interface StructuredMessageCardProps {
  data: Record<string, unknown>
  rawText: string
}

// ── Helpers ────────────────────────────────────────────────────────────────

function isTaskAssignment(d: Record<string, unknown>): boolean {
  return d.type === 'task_assignment' && typeof d.subject === 'string'
}

/** Minimal syntax-colored JSON — no dependencies */
function JsonBlock({ data }: { data: unknown }) {
  const json = JSON.stringify(data, null, 2)
  return (
    <pre className="text-xs font-mono leading-relaxed overflow-auto max-h-64 whitespace-pre-wrap break-all text-gray-600 dark:text-gray-400">
      {json}
    </pre>
  )
}

// ── Card ───────────────────────────────────────────────────────────────────

export function StructuredMessageCard({ data, rawText }: StructuredMessageCardProps) {
  const [jsonMode, setJsonMode] = useState(false)

  // Resolve chip + label + body based on message type
  let chip: string
  let chipColor: string
  let label: string
  let body: string | undefined

  if (isTaskAssignment(data)) {
    const task = data as unknown as TaskAssignment
    chip = `Task #${task.taskId}`
    chipColor = 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300'
    label = task.subject
    body = task.description
  } else {
    // Unknown structured type — show type as chip, first string field as label
    const typeName = typeof data.type === 'string' ? data.type : 'message'
    chip = typeName.replace(/_/g, ' ')
    chipColor = 'bg-gray-500/10 dark:bg-gray-500/20 text-gray-700 dark:text-gray-300'
    label =
      (typeof data.subject === 'string' && data.subject) ||
      (typeof data.summary === 'string' && data.summary) ||
      (typeof data.description === 'string' && data.description) ||
      rawText.slice(0, 80)
    body = typeof data.description === 'string' ? data.description : undefined
  }

  const hasBody = !!body || jsonMode

  return (
    <div
      className={cn(
        'overflow-hidden rounded-lg border transition-colors duration-200',
        'shadow-[0_1px_2px_rgba(0,0,0,0.04)] dark:shadow-[0_1px_3px_rgba(0,0,0,0.2)]',
        'border-gray-200/30 dark:border-gray-700/30',
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2">
        <span className="w-1.5 h-1.5 rounded-full flex-shrink-0 bg-blue-500" />
        <span
          className={cn(
            'inline-flex items-center px-2 py-0.5 rounded text-xs font-medium flex-shrink-0',
            chipColor,
          )}
        >
          {chip}
        </span>
        <span className="text-xs text-gray-600 dark:text-gray-300 truncate" title={label}>
          {label}
        </span>
        <span className="flex-1" />
        <button
          type="button"
          onClick={() => setJsonMode((v) => !v)}
          className={cn(
            'text-xs font-mono px-1.5 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
            'min-w-[28px] min-h-[22px] inline-flex items-center justify-center',
            jsonMode
              ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20 hover:bg-amber-500/25'
              : 'text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-400',
          )}
          title={jsonMode ? 'Rich view' : 'JSON view'}
        >
          {'{ }'}
        </button>
      </div>

      {/* Body — grid-template-rows for smooth expand */}
      <div
        className="grid transition-[grid-template-rows] duration-200 ease-out"
        style={{ gridTemplateRows: hasBody ? '1fr' : '0fr' }}
      >
        <div className="overflow-hidden">
          <div className="border-t border-gray-200/20 dark:border-gray-700/20 px-3 py-2">
            {jsonMode ? (
              <JsonBlock data={data} />
            ) : (
              body && (
                <p className="text-xs text-gray-600 dark:text-gray-300 whitespace-pre-wrap">
                  {body}
                </p>
              )
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
