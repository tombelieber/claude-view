import { useState, useMemo, useCallback } from 'react'
import { ChevronRight, ChevronDown, Copy, Check } from 'lucide-react'
import * as Tooltip from '@radix-ui/react-tooltip'
import { CompactCodeBlock } from './CompactCodeBlock'
import { useMonitorStore } from '../../store/monitor-store'

interface JsonTreeProps {
  data: unknown
  defaultExpandDepth?: number
  maxNodes?: number
  verboseMode?: boolean
}

const MAX_STRING_INLINE = 120
const DEFAULT_MAX_NODES = 200

function countNodes(data: unknown): number {
  if (data === null || data === undefined || typeof data !== 'object') return 1
  if (Array.isArray(data)) return 1 + data.reduce((sum: number, item) => sum + countNodes(item), 0)
  return 1 + Object.values(data as Record<string, unknown>).reduce((sum: number, val) => sum + countNodes(val), 0)
}

function JsonValue({ value, path, depth, defaultExpandDepth, expandedPaths, togglePath, verboseMode }: {
  value: unknown
  path: string
  depth: number
  defaultExpandDepth: number
  expandedPaths: Set<string>
  togglePath: (p: string) => void
  verboseMode?: boolean
}) {
  if (value === null) return <span className="text-gray-500">null</span>
  if (value === undefined) return <span className="text-gray-500">undefined</span>

  if (typeof value === 'string') {
    const display = value.length > MAX_STRING_INLINE
      ? `"${value.slice(0, MAX_STRING_INLINE)}\u2026"`
      : `"${value}"`
    const needsTooltip = value.length > MAX_STRING_INLINE

    const span = (
      <span className="text-green-600 dark:text-green-400 break-all">{display}</span>
    )

    if (!needsTooltip) return span

    return (
      <Tooltip.Root delayDuration={200}>
        <Tooltip.Trigger asChild>{span}</Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            align="start"
            className="z-50 max-w-md max-h-48 overflow-auto px-2 py-1.5 rounded bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 text-[10px] font-mono whitespace-pre-wrap break-all shadow-lg"
            sideOffset={4}
          >
            {value}
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    )
  }

  if (typeof value === 'number') return <span className="text-orange-600 dark:text-orange-400">{String(value)}</span>
  if (typeof value === 'boolean') return <span className="text-purple-600 dark:text-purple-400">{String(value)}</span>

  if (Array.isArray(value)) {
    return (
      <JsonCollapsible
        entries={value.map((item, i) => [String(i), item])}
        bracketOpen="["
        bracketClose="]"
        summaryLabel={`${value.length} items`}
        path={path}
        depth={depth}
        defaultExpandDepth={defaultExpandDepth}
        expandedPaths={expandedPaths}
        togglePath={togglePath}
        isArray
        verboseMode={verboseMode}
      />
    )
  }

  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>)
    return (
      <JsonCollapsible
        entries={entries}
        bracketOpen="{"
        bracketClose="}"
        summaryLabel={`${entries.length} keys`}
        path={path}
        depth={depth}
        defaultExpandDepth={defaultExpandDepth}
        expandedPaths={expandedPaths}
        togglePath={togglePath}
        verboseMode={verboseMode}
      />
    )
  }

  return <span className="text-gray-500">{String(value)}</span>
}

function JsonCollapsible({ entries, bracketOpen, bracketClose, summaryLabel, path, depth, defaultExpandDepth, expandedPaths, togglePath, isArray, verboseMode }: {
  entries: [string, unknown][]
  bracketOpen: string
  bracketClose: string
  summaryLabel: string
  path: string
  depth: number
  defaultExpandDepth: number
  expandedPaths: Set<string>
  togglePath: (p: string) => void
  isArray?: boolean
  verboseMode?: boolean
}) {
  const autoExpand = verboseMode ? depth < defaultExpandDepth : (depth < defaultExpandDepth && entries.length <= 5)
  const isExpanded = expandedPaths.has(path) ? true : (!expandedPaths.has(`~${path}`) && autoExpand)

  const toggle = () => {
    if (isExpanded) {
      // Collapse: mark with ~ prefix
      togglePath(`~${path}`)
    } else {
      // Expand: remove ~ prefix if exists, add path
      togglePath(path)
    }
  }

  if (entries.length === 0) {
    return <span className="text-gray-400 dark:text-gray-500">{bracketOpen}{bracketClose}</span>
  }

  if (!isExpanded) {
    return (
      <span className="inline-flex items-center gap-0.5">
        <button onClick={toggle} className="inline-flex items-center text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors">
          <ChevronRight className="w-3 h-3" />
          <span>{bracketOpen}</span>
        </button>
        <span className="text-[10px] text-gray-400 dark:text-gray-500 italic px-1">{summaryLabel}</span>
        <span className="text-gray-400 dark:text-gray-500">{bracketClose}</span>
      </span>
    )
  }

  return (
    <span>
      <button onClick={toggle} className="inline-flex items-center text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors">
        <ChevronDown className="w-3 h-3" />
        <span>{bracketOpen}</span>
      </button>
      <div className="pl-3 ml-1 border-l border-gray-200 dark:border-gray-700/40">
        {entries.map(([key, val], i) => (
          <div key={key} className="leading-relaxed">
            {!isArray && <span className="text-sky-600 dark:text-sky-400">{key}</span>}
            {!isArray && <span className="text-gray-400 dark:text-gray-500">: </span>}
            {isArray && <span className="text-gray-400 dark:text-gray-500 text-[9px] mr-1 select-none">{key}</span>}
            <JsonValue
              value={val}
              path={`${path}.${key}`}
              depth={depth + 1}
              defaultExpandDepth={defaultExpandDepth}
              expandedPaths={expandedPaths}
              togglePath={togglePath}
              verboseMode={verboseMode}
            />
            {i < entries.length - 1 && <span className="text-gray-400 dark:text-gray-500">,</span>}
          </div>
        ))}
      </div>
      <span className="text-gray-400 dark:text-gray-500">{bracketClose}</span>
    </span>
  )
}

export function JsonTree({ data, defaultExpandDepth = 2, maxNodes = DEFAULT_MAX_NODES, verboseMode: verboseModeProp }: JsonTreeProps) {
  const storeVerbose = useMonitorStore((s) => s.verboseMode)
  const verboseMode = verboseModeProp ?? storeVerbose
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())
  const [showFallback, setShowFallback] = useState(false)
  const [copied, setCopied] = useState(false)

  const effectiveExpandDepth = verboseMode ? 100 : defaultExpandDepth

  const nodeCount = useMemo(() => countNodes(data), [data])
  const isLarge = nodeCount > maxNodes

  const togglePath = (path: string) => {
    setExpandedPaths(prev => {
      const next = new Set(prev)
      if (path.startsWith('~')) {
        const realPath = path.slice(1)
        next.delete(realPath)
        next.add(path)
      } else {
        next.delete(`~${path}`)
        next.add(path)
      }
      return next
    })
  }

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(data, null, 2))
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy JSON:', err)
    }
  }, [data])

  if (isLarge && !showFallback) {
    return (
      <div className="text-[11px] font-mono">
        <span className="text-gray-500 dark:text-gray-400">Large object ({nodeCount} nodes) — </span>
        <button
          onClick={() => setShowFallback(true)}
          className="text-sky-600 dark:text-sky-400 hover:underline"
        >
          show as formatted JSON
        </button>
      </div>
    )
  }

  if (showFallback) {
    return (
      <div>
        <button
          onClick={() => setShowFallback(false)}
          className="text-[10px] text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 mb-1 transition-colors"
        >
          [ Show tree view ]
        </button>
        <CompactCodeBlock code={JSON.stringify(data, null, 2)} language="json" />
      </div>
    )
  }

  return (
    <Tooltip.Provider delayDuration={200}>
      <div className="text-[11px] font-mono leading-relaxed relative">
        <button
          onClick={handleCopy}
          className="absolute top-0 right-0 p-1 rounded text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors z-10"
          title="Copy JSON"
        >
          {copied ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3" />}
        </button>
        <JsonValue
          value={data}
          path="$"
          depth={0}
          defaultExpandDepth={effectiveExpandDepth}
          expandedPaths={expandedPaths}
          togglePath={togglePath}
          verboseMode={verboseMode}
        />
      </div>
    </Tooltip.Provider>
  )
}
