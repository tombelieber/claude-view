import { type ReactNode, useState } from 'react'
import { useDeveloperTools } from '../../../../contexts/DeveloperToolsContext'
import { cn } from '../../../../utils/cn'
import { useJsonMode } from './json-mode-context'
import { SimpleJsonView } from './SimpleJsonView'

/**
 * Shared card wrapper for all developer-mode event blocks.
 *   [status dot] [colored chip] [label]    [{ }] [meta]
 *   [children OR raw JSON when toggled]
 *
 * When `rawData` is provided, a `{ }` toggle appears in the header.
 * Click it to switch between the tailored UI (children) and raw JSON dump.
 *
 * When `label` is long (>80 chars) and no children/rawJSON body,
 * the full label is auto-shown in the body.
 */

// ── Status dot ──────────────────────────────────────────────────────────────

type DotColor =
  | 'green'
  | 'red'
  | 'amber'
  | 'blue'
  | 'gray'
  | 'cyan'
  | 'teal'
  | 'orange'
  | 'indigo'
  | 'purple'

const DOT_COLORS: Record<DotColor, string> = {
  green: 'bg-green-500',
  red: 'bg-red-500',
  amber: 'bg-amber-400',
  blue: 'bg-blue-500',
  gray: 'bg-gray-400',
  cyan: 'bg-cyan-500',
  teal: 'bg-teal-500',
  orange: 'bg-orange-500',
  indigo: 'bg-indigo-500',
  purple: 'bg-purple-500',
}

// ── Chip colors ─────────────────────────────────────────────────────────────

const CHIP_COLORS: Record<string, string> = {
  error: 'bg-red-500/10 dark:bg-red-500/20 text-red-700 dark:text-red-300',
  warning: 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300',
  info: 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300',
  success: 'bg-green-500/10 dark:bg-green-500/20 text-green-700 dark:text-green-300',
  system: 'bg-cyan-500/10 dark:bg-cyan-500/20 text-cyan-700 dark:text-cyan-300',
  hook: 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300',
  agent: 'bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300',
  mcp: 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300',
  builtin: 'bg-gray-500/10 dark:bg-gray-500/20 text-gray-700 dark:text-gray-300',
  queue: 'bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300',
  snapshot: 'bg-teal-500/10 dark:bg-teal-500/20 text-teal-700 dark:text-teal-300',
  progress: 'bg-purple-500/10 dark:bg-purple-500/20 text-purple-700 dark:text-purple-300',
}

const LABEL_TRUNCATE_AT = 80

// ── Component ───────────────────────────────────────────────────────────────

interface EventCardProps {
  dot: DotColor
  chip: string
  chipColor?: string
  label?: string
  meta?: ReactNode
  children?: ReactNode
  /** Raw block data — when provided, a { } toggle appears to switch to JSON view */
  rawData?: unknown
  /** Error-style border */
  error?: boolean
  /** Pulse the status dot */
  pulse?: boolean
}

export function EventCard({
  dot,
  chip,
  chipColor,
  label,
  meta,
  children,
  rawData,
  error,
  pulse,
}: EventCardProps) {
  const { JsonTree } = useDeveloperTools()
  const globalJsonMode = useJsonMode()
  const [localOverride, setLocalOverride] = useState<boolean | null>(null)
  const jsonMode = localOverride ?? globalJsonMode
  const resolvedChipColor = chipColor ?? CHIP_COLORS[chip.toLowerCase()] ?? CHIP_COLORS.system
  const hasRawData = rawData !== undefined && rawData !== null

  // Auto-expand: if label is long and no explicit children/rawData body
  const labelOverflows = label != null && label.length > LABEL_TRUNCATE_AT
  const autoExpandBody = labelOverflows && !children && !jsonMode

  // Determine what body to show
  const showJsonBody = jsonMode && hasRawData
  const showChildrenBody = !jsonMode && children
  const hasBody = showJsonBody || showChildrenBody || autoExpandBody

  return (
    <div
      className={cn(
        'overflow-hidden rounded-lg border transition-colors duration-200',
        error
          ? 'border-red-300/25 dark:border-red-800/40 bg-red-500/5 dark:bg-red-950/20'
          : 'border-gray-200/30 dark:border-gray-700/30',
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2">
        <span
          className={cn(
            'w-1.5 h-1.5 rounded-full flex-shrink-0',
            DOT_COLORS[dot],
            pulse && 'animate-pulse',
          )}
        />
        <span
          className={cn(
            'inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold flex-shrink-0',
            resolvedChipColor,
          )}
        >
          {chip}
        </span>
        {label && (
          <span
            className="text-xs text-gray-400 dark:text-gray-500 font-mono truncate"
            title={label}
          >
            {label}
          </span>
        )}
        <span className="flex-1" />
        {hasRawData && (
          <button
            type="button"
            onClick={() => setLocalOverride((v) => !(v ?? globalJsonMode))}
            className={cn(
              'text-[10px] font-mono px-1.5 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
              jsonMode
                ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20'
                : 'text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-400',
            )}
            title={jsonMode ? 'Switch to rich view' : 'Switch to JSON view'}
          >
            {'{ }'}
          </button>
        )}
        {meta}
      </div>

      {/* Body */}
      {hasBody && (
        <div className="border-t border-gray-200/20 dark:border-gray-700/20 px-3 py-2">
          {showJsonBody &&
            (JsonTree ? (
              <JsonTree data={rawData} defaultExpandDepth={3} verboseMode />
            ) : (
              <SimpleJsonView data={rawData} />
            ))}
          {showChildrenBody && children}
          {autoExpandBody && (
            <p className="text-sm text-gray-600 dark:text-gray-300 whitespace-pre-wrap break-words">
              {label}
            </p>
          )}
        </div>
      )}
    </div>
  )
}
