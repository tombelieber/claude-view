import { Check, Copy, Play } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { useShikiHighlighter } from '../../hooks/use-shiki'
import { useTheme } from '../../hooks/use-theme'
import { cn } from '../../lib/utils'
import type { WorkflowDefinition } from '../../types/generated/WorkflowDefinition'
import { WorkflowDiagram } from './WorkflowDiagram'

function YamlDisplay({ code }: { code: string }) {
  const highlighter = useShikiHighlighter()
  const { resolvedTheme } = useTheme()
  const [copied, setCopied] = useState(false)

  const shikiTheme = resolvedTheme === 'dark' ? 'github-dark' : 'github-light'

  const highlightedHtml = useMemo(() => {
    if (!highlighter || !code) return null
    try {
      return highlighter.codeToHtml(code, { lang: 'yaml', theme: shikiTheme })
    } catch {
      return null
    }
  }, [highlighter, code, shikiTheme])

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // ignore
    }
  }, [code])

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between px-5 py-3 border-b border-[#D1D1D6] dark:border-[#3A3A3C] shrink-0">
        <span className="text-[12px] font-medium text-[#AEAEB2] dark:text-[#636366]">
          workflow.yaml
        </span>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[12px] font-medium
                     text-[#6E6E73] dark:text-[#98989D]
                     hover:bg-black/[0.06] dark:hover:bg-white/[0.08]
                     transition-colors cursor-pointer"
        >
          {copied ? (
            <>
              <Check className="w-3 h-3 text-[#22C55E]" />
              <span className="text-[#22C55E]">Copied</span>
            </>
          ) : (
            <>
              <Copy className="w-3 h-3" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      <div className="flex-1 overflow-auto">
        {highlightedHtml ? (
          <div
            className="p-5 text-[13px] [&_pre]:!m-0 [&_pre]:!p-0 [&_pre]:!bg-transparent [&_code]:!bg-transparent [&_pre]:!text-[13px]"
            // biome-ignore lint/security/noDangerouslySetInnerHtml: Shiki renders YAML from local disk — no user-submitted content
            dangerouslySetInnerHTML={{ __html: highlightedHtml }}
          />
        ) : (
          <pre className="p-5 text-[13px] text-[#1D1D1F] dark:text-white font-mono m-0 leading-relaxed">
            <code>{code}</code>
          </pre>
        )}
      </div>
    </div>
  )
}

function StageList({ def }: { def: WorkflowDefinition }) {
  return (
    <div className="flex flex-col overflow-auto py-2">
      <div className="flex items-center gap-3 px-5 py-2.5">
        <div className="w-2 h-2 rounded-full bg-[#22C55E] shrink-0" />
        <span className="text-[13px] text-[#6E6E73] dark:text-[#98989D]">
          {def.inputs[0]?.name ?? 'Input'}
        </span>
      </div>

      {def.stages.map((stage, i) => (
        <div key={stage.name} className="flex flex-col">
          <div className="flex items-start gap-3 px-5">
            <div className="ml-[3px] w-px h-4 bg-[#D1D1D6] dark:bg-[#3A3A3C] shrink-0" />
          </div>
          <div className="flex items-start gap-3 px-5 py-2.5 hover:bg-black/[0.03] dark:hover:bg-white/[0.03] rounded-lg mx-2 transition-colors cursor-default">
            <div className="w-2 h-2 rounded-sm bg-[#1D1D1F]/20 dark:bg-white/20 shrink-0 mt-1" />
            <div className="flex flex-col gap-1 min-w-0">
              <span className="text-[13px] font-medium text-[#1D1D1F] dark:text-white leading-tight">
                {stage.name}
              </span>
              <div className="flex flex-wrap gap-1">
                {stage.skills.map((skill) => (
                  <span
                    key={skill}
                    className="text-[11px] px-1.5 py-0.5 rounded-md bg-[#E5E5EA] dark:bg-[#2C2C2E] text-[#6E6E73] dark:text-[#98989D]"
                  >
                    {skill}
                  </span>
                ))}
              </div>
              {stage.parallel && <span className="text-[11px] text-[#22C55E]">Parallel</span>}
            </div>
            <span className="ml-auto text-[11px] text-[#C7C7CC] dark:text-[#48484A] shrink-0 mt-0.5">
              {i + 1}
            </span>
          </div>
        </div>
      ))}

      <div className="flex items-start gap-3 px-5">
        <div className="ml-[3px] w-px h-4 bg-[#D1D1D6] dark:bg-[#3A3A3C] shrink-0" />
      </div>
      <div className="flex items-center gap-3 px-5 py-2.5">
        <div className="w-2 h-2 rounded-full border border-[#C7C7CC] dark:border-[#48484A] shrink-0" />
        <span className="text-[13px] text-[#AEAEB2] dark:text-[#636366]">Done</span>
      </div>
    </div>
  )
}

interface WorkflowPreviewTabProps {
  definition: WorkflowDefinition | null
  yaml: string
  onGenerate: () => void
  canGenerate: boolean
}

export function WorkflowPreviewTab({
  definition,
  yaml,
  onGenerate,
  canGenerate,
}: WorkflowPreviewTabProps) {
  const { resolvedTheme } = useTheme()
  const [view, setView] = useState<'diagram' | 'yaml'>('diagram')

  if (!definition) {
    return (
      <div className="flex items-center justify-center h-full bg-[#F5F5F7] dark:bg-[#000000]">
        <div className="text-center">
          <p className="text-[15px] font-medium text-[#1D1D1F] dark:text-white mb-1">
            No workflow selected
          </p>
          <p className="text-[13px] text-[#6E6E73] dark:text-[#98989D]">
            Go back and choose a workflow to preview.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full overflow-hidden bg-[#F5F5F7] dark:bg-[#000000]">
      <div className="flex flex-1 overflow-hidden min-h-0">
        {/* Left: stage list */}
        <div className="w-56 shrink-0 border-r border-[#D1D1D6] dark:border-[#3A3A3C] overflow-hidden flex flex-col bg-white dark:bg-[#1C1C1E]">
          <div className="px-5 py-3 border-b border-[#D1D1D6] dark:border-[#3A3A3C] shrink-0">
            <span className="text-[11px] font-semibold tracking-wider text-[#AEAEB2] dark:text-[#636366] uppercase">
              {definition.stages.length} Stages
            </span>
          </div>
          <div className="flex-1 overflow-hidden">
            <StageList def={definition} />
          </div>
        </div>

        {/* Right: diagram / yaml */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Sub-tabs */}
          <div className="flex items-center gap-0 px-5 border-b border-[#D1D1D6] dark:border-[#3A3A3C] bg-white dark:bg-[#1C1C1E] shrink-0">
            {(['diagram', 'yaml'] as const).map((v) => (
              <button
                key={v}
                type="button"
                onClick={() => setView(v)}
                className={cn(
                  'relative px-1 mr-4 py-3 text-[13px] font-medium capitalize',
                  'transition-colors duration-150 cursor-pointer',
                  'after:absolute after:bottom-0 after:left-0 after:right-0 after:h-[2px] after:rounded-full after:transition-all after:duration-150',
                  view === v
                    ? 'text-[#1D1D1F] dark:text-white after:bg-[#1D1D1F] dark:after:bg-white'
                    : 'text-[#6E6E73] dark:text-[#98989D] after:bg-transparent hover:text-[#1D1D1F] dark:hover:text-white',
                )}
              >
                {v}
              </button>
            ))}
          </div>

          {/* Content */}
          <div className="flex-1 overflow-hidden bg-white dark:bg-[#1C1C1E]">
            {view === 'diagram' ? (
              <WorkflowDiagram definition={definition} isDark={resolvedTheme === 'dark'} />
            ) : (
              <div className="h-full overflow-hidden">
                {yaml ? (
                  <YamlDisplay code={yaml} />
                ) : (
                  <div className="flex items-center justify-center h-full text-[13px] text-[#AEAEB2] dark:text-[#636366]">
                    No YAML available
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Footer */}
      <div className="shrink-0 px-5 py-4 border-t border-[#D1D1D6] dark:border-[#3A3A3C] bg-white dark:bg-[#1C1C1E] flex items-center justify-between gap-4">
        <p className="text-[13px] text-[#6E6E73] dark:text-[#98989D] truncate leading-relaxed">
          {definition.description}
        </p>
        <button
          type="button"
          onClick={onGenerate}
          disabled={!canGenerate}
          className={cn(
            'flex items-center gap-2 px-5 py-2 rounded-full text-[13px] font-semibold',
            'transition-all duration-150 cursor-pointer shrink-0 active:scale-95',
            canGenerate
              ? 'bg-[#22C55E] text-white hover:bg-[#16A34A]'
              : 'bg-[#E5E5EA] dark:bg-[#2C2C2E] text-[#AEAEB2] dark:text-[#636366] cursor-not-allowed',
          )}
        >
          <Play className="w-3.5 h-3.5" fill="currentColor" />
          Run Workflow
        </button>
      </div>
    </div>
  )
}
