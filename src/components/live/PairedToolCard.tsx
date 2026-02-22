import { useState, useMemo } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeRaw from 'rehype-raw'
import { Wrench, CheckCircle, XCircle, Loader2 } from 'lucide-react'
import { CompactCodeBlock } from './CompactCodeBlock'
import { JsonTree } from './JsonTree'
import { AskUserQuestionDisplay, isAskUserQuestionInput } from './AskUserQuestionDisplay'
import { getToolRenderer } from './ToolRenderers'
import { useMonitorStore } from '../../store/monitor-store'
import { cn } from '../../lib/utils'
import {
  tryParseJson, isJsonContent, isDiffContent, isCodeLikeContent,
  stripLineNumbers, detectCodeLanguage, shortenToolName, toolChipColor,
} from '../../lib/content-detection'
import { markdownComponents } from '../../lib/markdown-components'
import type { RichMessage } from './RichPane'

// --- Helpers ---

/** Format a timestamp as a static time label (chat-app style). Guards against epoch-zero. */
function formatTimestamp(ts: number | undefined): string | null {
  if (!ts || ts <= 0) return null
  const date = new Date(ts * 1000)
  if (isNaN(date.getTime())) return null
  const now = new Date()
  const time = date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
  if (date.toDateString() === now.toDateString()) return time
  const yesterday = new Date(now)
  yesterday.setDate(yesterday.getDate() - 1)
  if (date.toDateString() === yesterday.toDateString()) return `Yesterday ${time}`
  if (date.getFullYear() === now.getFullYear()) {
    const month = date.toLocaleString(undefined, { month: 'short' })
    return `${month} ${date.getDate()}, ${time}`
  }
  const month = date.toLocaleString(undefined, { month: 'short' })
  return `${month} ${date.getDate()} '${String(date.getFullYear()).slice(-2)}, ${time}`
}

function Timestamp({ ts }: { ts?: number }) {
  const label = formatTimestamp(ts)
  if (!label) return null
  return (
    <span className="text-[9px] text-gray-400 dark:text-gray-600 tabular-nums flex-shrink-0 whitespace-nowrap">
      {label}
    </span>
  )
}

/** Compute duration between two timestamps in seconds. */
function computeDuration(startTs?: number, endTs?: number): string | null {
  if (!startTs || !endTs || startTs <= 0 || endTs <= 0) return null
  const diff = endTs - startTs
  const MAX_TOOL_DURATION_S = 300 // 5 minutes — ignore implausible durations
  if (diff < 0 || diff > MAX_TOOL_DURATION_S) return null
  if (diff < 1) return `${Math.round(diff * 1000)}ms`
  return `${diff.toFixed(1)}s`
}

// --- Result Content Renderer ---

function ResultContent({ content, index, verboseMode }: { content: string; index: number; verboseMode: boolean }) {
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const hasContent = content.length > 0
  if (!hasContent) return null

  const jsonDetected = isJsonContent(content)
  const diffLike = !jsonDetected && isDiffContent(content)
  const codeLike = !jsonDetected && !diffLike && isCodeLikeContent(content)
  const codeLang = codeLike ? detectCodeLanguage(content) : 'text'
  const cleanCode = codeLike ? stripLineNumbers(content) : content
  const parsedJson = jsonDetected ? tryParseJson(content) : null

  if (jsonDetected && parsedJson !== null) {
    return richRenderMode === 'json' ? (
      <CompactCodeBlock code={JSON.stringify(parsedJson, null, 2)} language="json" blockId={`result-${index}`} />
    ) : (
      <JsonTree data={parsedJson} />
    )
  }
  if (diffLike) {
    return (
      <div className="diff-block">
        <CompactCodeBlock code={content} language="diff" blockId={`result-${index}`} />
      </div>
    )
  }
  if (codeLike) {
    return <CompactCodeBlock code={cleanCode} language={codeLang} blockId={`result-${index}`} />
  }
  return (
    <div className="text-[10px] text-gray-600 dark:text-gray-500 font-mono leading-relaxed prose dark:prose-invert prose-sm max-w-none">
      <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{content}</Markdown>
    </div>
  )
}

// --- Main Component ---

export interface PairedToolCardProps {
  toolUse: RichMessage
  toolResult: RichMessage | null
  index: number
  verboseMode?: boolean
}

export function PairedToolCard({ toolUse, toolResult, index, verboseMode = false }: PairedToolCardProps) {
  const [cardOverride, setCardOverride] = useState<'rich' | 'json' | null>(null)
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const rawName = toolUse.name || 'Tool'
  const { short: label, server } = shortenToolName(rawName)
  const chipColor = toolChipColor(rawName)
  const inputObj = toolUse.inputData
  const isObjectInput = inputObj !== null && inputObj !== undefined && typeof inputObj === 'object' && !Array.isArray(inputObj)
  const isAskUserQuestion = rawName === 'AskUserQuestion' && isAskUserQuestionInput(inputObj)

  const RichRenderer = getToolRenderer(rawName)
  const effectiveMode = cardOverride ?? richRenderMode
  const showRich = !!RichRenderer && effectiveMode === 'rich' && isObjectInput

  const duration = useMemo(
    () => computeDuration(toolUse.ts, toolResult?.ts),
    [toolUse.ts, toolResult?.ts],
  )

  // Detect if result is an error (heuristic: content contains error-like patterns)
  const isError = useMemo(() => {
    if (!toolResult) return false
    const c = toolResult.content.toLowerCase()
    return c.startsWith('error') || c.startsWith('failed') || c.includes('permission denied')
  }, [toolResult])

  // AskUserQuestion special case in non-verbose mode
  if (isAskUserQuestion && !verboseMode) {
    return (
      <div className="py-0.5">
        <AskUserQuestionDisplay inputData={inputObj} variant="amber" />
      </div>
    )
  }

  return (
    <div className="py-0.5 border-l-2 border-orange-500/30 dark:border-orange-500/20 pl-1">
      {/* Header */}
      <div className="flex items-start gap-1.5">
        <Wrench className="w-3 h-3 text-orange-500 dark:text-orange-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          {/* Tool name + server + toggle */}
          <div className="flex items-start gap-1.5 flex-wrap">
            <span className={`inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold flex-shrink-0 ${chipColor}`}>
              {label}
            </span>
            {server && (
              <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600 flex-shrink-0 self-center">
                {server}
              </span>
            )}
            {/* Per-card rich/json toggle */}
            {RichRenderer && (
              <button
                onClick={() => setCardOverride(effectiveMode === 'rich' ? 'json' : 'rich')}
                className={cn(
                  'text-[10px] font-mono px-1 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
                  effectiveMode === 'json'
                    ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20'
                    : 'text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-400',
                )}
                title={effectiveMode === 'rich' ? 'Switch to JSON view' : 'Switch to rich view'}
              >
                {'{ }'}
              </button>
            )}
          </div>

          {/* Input section */}
          {showRich ? (
            <div className="mt-1">
              <RichRenderer inputData={inputObj as Record<string, unknown>} name={rawName} blockIdPrefix={`${index}-`} />
            </div>
          ) : isObjectInput ? (
            <div className="mt-1">
              <CompactCodeBlock code={JSON.stringify(inputObj, null, 2)} language="json" blockId={`tool-input-${index}`} />
            </div>
          ) : toolUse.input ? (
            <div className="mt-1">
              <CompactCodeBlock code={toolUse.input} language="json" blockId={`tool-input-${index}`} />
            </div>
          ) : null}

          {/* Result section */}
          {toolResult ? (
            <div className="mt-1 pt-1 border-t border-gray-200/50 dark:border-gray-700/50">
              <div className="flex items-center gap-1.5 mb-0.5">
                {isError ? (
                  <XCircle className="w-3 h-3 text-red-500 dark:text-red-400 flex-shrink-0" />
                ) : (
                  <CheckCircle className="w-3 h-3 text-green-500 dark:text-green-400 flex-shrink-0" />
                )}
                <span className={cn(
                  'text-[10px] font-mono',
                  isError ? 'text-red-500 dark:text-red-400' : 'text-gray-500 dark:text-gray-600',
                )}>
                  {isError ? 'error' : 'result'}
                </span>
                {duration && (
                  <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600 px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800">
                    {duration}
                  </span>
                )}
              </div>
              <div className="pl-4">
                <ResultContent content={toolResult.content} index={index} verboseMode={verboseMode} />
              </div>
            </div>
          ) : (
            <div className="mt-1 pt-1 border-t border-gray-200/50 dark:border-gray-700/50">
              <div className="flex items-center gap-1.5">
                <Loader2 className="w-3 h-3 text-gray-400 dark:text-gray-600 animate-spin flex-shrink-0" />
                <span className="text-[10px] text-gray-400 dark:text-gray-600 italic">pending...</span>
              </div>
            </div>
          )}
        </div>
        <Timestamp ts={toolUse.ts} />
      </div>
    </div>
  )
}
