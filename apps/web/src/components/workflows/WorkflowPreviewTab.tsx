import { Play } from 'lucide-react'
import { useState } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { cn } from '../../lib/utils'
import type { WorkflowDefinition } from '../../types/generated/WorkflowDefinition'
import { MermaidRenderer } from './MermaidRenderer'

function sanitizeId(s: string) {
  return s.replace(/[^a-zA-Z0-9]/g, '_')
}

function buildMermaid(def: WorkflowDefinition): string {
  const lines = ['graph LR']
  const inputId = 'Input'
  lines.push(
    `  ${inputId}([${def.inputs[0]?.name ?? 'Input'}]) --> ${sanitizeId(def.stages[0]?.name ?? 'Stage')}`,
  )
  for (let i = 0; i < def.stages.length; i++) {
    const stage = def.stages[i]
    const sid = sanitizeId(stage.name)
    const skills = stage.skills.join('\\n')
    const gate = stage.gate ? `\\nGate: ${stage.gate.condition}` : ''
    lines.push(`  ${sid}{${stage.name}\\n${skills}${gate}}`)
    if (stage.gate?.retry) lines.push(`  ${sid} -->|fail| ${sid}`)
    const next = def.stages[i + 1]
    if (next) lines.push(`  ${sid} -->|pass| ${sanitizeId(next.name)}`)
    else lines.push(`  ${sid} -->|pass| Done([Done])`)
  }
  return lines.join('\n')
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
  const [isPulsing, setIsPulsing] = useState(false)
  const chart = definition ? buildMermaid(definition) : ''

  function handleGenerate() {
    setIsPulsing(true)
    setTimeout(() => {
      setIsPulsing(false)
      onGenerate()
    }, 300)
  }

  return (
    <div className="flex flex-col h-full">
      <div
        className={cn(
          'flex-[3] overflow-auto border-b border-gray-200 dark:border-gray-800 p-4',
          isPulsing && 'animate-pulse',
        )}
      >
        {chart ? (
          <MermaidRenderer chart={chart} />
        ) : (
          <div className="flex items-center justify-center h-full text-sm text-gray-400 dark:text-gray-600">
            Describe your workflow in the chat to see a preview here.
          </div>
        )}
      </div>
      <div className="flex-[2] overflow-auto p-4">
        {yaml && <Markdown remarkPlugins={[remarkGfm]}>{`\`\`\`yaml\n${yaml}\n\`\`\``}</Markdown>}
      </div>
      <div className="p-3 border-t border-gray-200 dark:border-gray-800 flex justify-end">
        <button
          type="button"
          onClick={handleGenerate}
          disabled={!canGenerate}
          className="flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium
                     bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900
                     hover:bg-gray-700 dark:hover:bg-gray-300
                     disabled:opacity-40 disabled:cursor-not-allowed
                     transition-colors cursor-pointer"
        >
          <Play className="w-4 h-4" />
          Generate Workflow
        </button>
      </div>
    </div>
  )
}
