import React from 'react'
import { CompactCodeBlock } from './CompactCodeBlock'
import { cn } from '../../lib/utils'
import { FileText, Search, Terminal, Link } from 'lucide-react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeRaw from 'rehype-raw'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ToolRendererProps {
  inputData: Record<string, unknown>
  name: string
  /** Unique prefix for blockIds to prevent collisions when the same tool appears multiple times. */
  blockIdPrefix?: string
}

// ---------------------------------------------------------------------------
// Helpers: Extension -> Language
// ---------------------------------------------------------------------------

const EXT_LANG: Record<string, string> = {
  rs: 'rust', ts: 'typescript', tsx: 'tsx', js: 'javascript',
  jsx: 'jsx', py: 'python', go: 'go', sql: 'sql',
  css: 'css', html: 'html', json: 'json', yaml: 'yaml',
  yml: 'yaml', toml: 'toml', md: 'markdown', sh: 'bash',
  bash: 'bash', zsh: 'bash', c: 'c', cpp: 'cpp',
  java: 'java', rb: 'ruby', swift: 'swift', kt: 'kotlin',
}

function langFromPath(filePath: string): string {
  const ext = filePath.split('.').pop()?.toLowerCase() || ''
  return EXT_LANG[ext] || 'text'
}

// ---------------------------------------------------------------------------
// Helpers: Inline Diff
// ---------------------------------------------------------------------------

function generateInlineDiff(oldStr: string, newStr: string): string {
  const oldLines = oldStr.split('\n')
  const newLines = newStr.split('\n')
  let diff = ''
  for (const line of oldLines) diff += `- ${line}\n`
  for (const line of newLines) diff += `+ ${line}\n`
  return diff.trimEnd()
}

// ---------------------------------------------------------------------------
// Helpers: Chip
// ---------------------------------------------------------------------------

export function Chip({ label, value, className }: { label: string; value: string | number; className?: string }) {
  return (
    <span className={cn(
      'inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
      className,
    )}>
      <span className="text-gray-400 dark:text-gray-600">{label}:</span>
      {value}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Helpers: File Path Header
// ---------------------------------------------------------------------------

export function FilePathHeader({ path }: { path: string }) {
  const ext = path.split('.').pop()?.toLowerCase() || ''
  return (
    <div className="flex items-center gap-1.5 mb-1">
      <FileText className="w-3 h-3 text-gray-400 dark:text-gray-600 flex-shrink-0" />
      <span className="text-[11px] font-mono text-gray-700 dark:text-gray-300 truncate">{path}</span>
      {ext && (
        <span className="text-[9px] font-mono px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-500 flex-shrink-0">
          .{ext}
        </span>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers: Inline Markdown
// ---------------------------------------------------------------------------

export function InlineMarkdown({ text }: { text: string }) {
  return (
    <div className="text-[11px] text-gray-700 dark:text-gray-300 prose prose-sm dark:prose-invert max-w-none [&_p]:m-0 [&_p]:leading-snug [&_ul]:my-1 [&_ol]:my-1 [&_li]:my-0">
      <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]}>{text}</Markdown>
    </div>
  )
}

// ---------------------------------------------------------------------------
// File Operation Renderers
// ---------------------------------------------------------------------------

function EditRenderer({ inputData, blockIdPrefix: p = '' }: ToolRendererProps) {
  const filePath = (inputData.file_path as string) || ''
  const oldString = (inputData.old_string as string) ?? ''
  const newString = (inputData.new_string as string) ?? ''
  const replaceAll = inputData.replace_all as boolean | undefined

  const hasDiff = oldString !== '' && newString !== ''
  const diffCode = hasDiff ? generateInlineDiff(oldString, newString) : null

  return (
    <div className="space-y-1">
      {filePath && <FilePathHeader path={filePath} />}
      <div className="flex items-center gap-1 flex-wrap">
        {replaceAll && (
          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300">
            replace_all
          </span>
        )}
      </div>
      {diffCode && (
        <CompactCodeBlock code={diffCode} language="diff" blockId={`${p}edit-diff-${filePath}`} />
      )}
      {!hasDiff && oldString && (
        <CompactCodeBlock code={oldString} language={langFromPath(filePath)} blockId={`${p}edit-old-${filePath}`} />
      )}
    </div>
  )
}

function WriteRenderer({ inputData, blockIdPrefix: p = '' }: ToolRendererProps) {
  const filePath = (inputData.file_path as string) || ''
  const content = (inputData.content as string) || ''
  const lang = filePath ? langFromPath(filePath) : 'text'

  return (
    <div className="space-y-1">
      {filePath && <FilePathHeader path={filePath} />}
      {content && (
        <CompactCodeBlock code={content} language={lang} blockId={`${p}write-${filePath}`} />
      )}
    </div>
  )
}

function ReadRenderer({ inputData }: ToolRendererProps) {
  const filePath = (inputData.file_path as string) || ''
  const offset = inputData.offset as number | undefined
  const limit = inputData.limit as number | undefined
  const pages = inputData.pages as string | undefined

  return (
    <div className="space-y-1">
      {filePath && <FilePathHeader path={filePath} />}
      <div className="flex items-center gap-1.5 flex-wrap">
        {offset != null && <Chip label="offset" value={offset} />}
        {limit != null && <Chip label="limit" value={limit} />}
        {pages && <Chip label="pages" value={pages} />}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Search Renderers
// ---------------------------------------------------------------------------

function GrepRenderer({ inputData }: ToolRendererProps) {
  const pattern = (inputData.pattern as string) || ''
  const path = inputData.path as string | undefined
  const glob = inputData.glob as string | undefined
  const type = inputData.type as string | undefined
  const outputMode = inputData.output_mode as string | undefined
  const contextA = inputData['-A'] as number | undefined
  const contextB = inputData['-B'] as number | undefined
  const contextC = inputData['-C'] as number | undefined

  return (
    <div className="space-y-1">
      {pattern && (
        <div className="flex items-center gap-1.5">
          <Search className="w-3 h-3 text-amber-500 dark:text-amber-400 flex-shrink-0" />
          <span className="text-[11px] font-mono bg-amber-500/10 dark:bg-amber-500/20 rounded px-1.5 py-0.5 text-amber-800 dark:text-amber-200">
            {pattern}
          </span>
        </div>
      )}
      <div className="flex items-center gap-1.5 flex-wrap">
        {path && <Chip label="path" value={path} />}
        {glob && <Chip label="glob" value={glob} />}
        {type && <Chip label="type" value={type} />}
        {outputMode && <Chip label="mode" value={outputMode} />}
        {contextA != null && (
          <span className="inline-flex items-center px-1 py-0.5 rounded text-[9px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
            -A {contextA}
          </span>
        )}
        {contextB != null && (
          <span className="inline-flex items-center px-1 py-0.5 rounded text-[9px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
            -B {contextB}
          </span>
        )}
        {contextC != null && (
          <span className="inline-flex items-center px-1 py-0.5 rounded text-[9px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
            -C {contextC}
          </span>
        )}
      </div>
    </div>
  )
}

function GlobRenderer({ inputData }: ToolRendererProps) {
  const pattern = (inputData.pattern as string) || ''
  const path = inputData.path as string | undefined

  return (
    <div className="space-y-1">
      {pattern && (
        <span className="text-[11px] font-mono text-gray-700 dark:text-gray-300">{pattern}</span>
      )}
      {path && (
        <div className="text-[10px] font-mono text-gray-500 dark:text-gray-500">{path}</div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Execution Renderers
// ---------------------------------------------------------------------------

function BashRenderer({ inputData, blockIdPrefix: p = '' }: ToolRendererProps) {
  const command = (inputData.command as string) || ''
  const description = inputData.description as string | undefined
  const timeout = inputData.timeout as number | undefined

  return (
    <div className="space-y-1">
      {description && (
        <div className="flex items-center gap-1.5">
          <Terminal className="w-3 h-3 text-gray-400 dark:text-gray-600 flex-shrink-0" />
          <span className="text-[10px] text-gray-500 dark:text-gray-400">{description}</span>
        </div>
      )}
      {command && (
        <CompactCodeBlock code={command} language="bash" blockId={`${p}bash-${command.slice(0, 40)}`} />
      )}
      {timeout != null && (
        <div className="flex items-center gap-1.5">
          <Chip label="timeout" value={`${timeout}ms`} />
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Agent / Orchestration Renderers
// ---------------------------------------------------------------------------

function TaskRenderer({ inputData }: ToolRendererProps) {
  const prompt = (inputData.prompt as string) || ''
  const subagentType = inputData.subagent_type as string | undefined
  const description = inputData.description as string | undefined
  const name = inputData.name as string | undefined
  const model = inputData.model as string | undefined
  const mode = inputData.mode as string | undefined

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-1.5 flex-wrap">
        {subagentType && (
          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300">
            {subagentType}
          </span>
        )}
        {name && (
          <span className="text-[11px] font-semibold text-gray-800 dark:text-gray-200">{name}</span>
        )}
        {model && <Chip label="model" value={model} />}
        {mode && <Chip label="mode" value={mode} />}
      </div>
      {description && (
        <div className="text-[10px] text-gray-500 dark:text-gray-400">{description}</div>
      )}
      {prompt && <InlineMarkdown text={prompt} />}
    </div>
  )
}

function SkillRenderer({ inputData }: ToolRendererProps) {
  const skill = (inputData.skill as string) || ''
  const args = inputData.args as string | undefined

  return (
    <div className="space-y-1">
      {skill && (
        <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-purple-500/10 dark:bg-purple-500/20 text-purple-700 dark:text-purple-300">
          {skill}
        </span>
      )}
      {args && <InlineMarkdown text={args} />}
    </div>
  )
}

function SendMessageRenderer({ inputData }: ToolRendererProps) {
  const recipient = inputData.recipient as string | undefined
  const content = (inputData.content as string) || ''
  const msgType = inputData.type as string | undefined

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-1.5 flex-wrap">
        {recipient && (
          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-indigo-100 dark:bg-indigo-900/30 text-indigo-700 dark:text-indigo-300">
            {recipient}
          </span>
        )}
        {msgType && <Chip label="type" value={msgType} />}
      </div>
      {content && <InlineMarkdown text={content} />}
    </div>
  )
}

function TaskCreateRenderer({ inputData }: ToolRendererProps) {
  const subject = (inputData.subject as string) || ''
  const description = (inputData.description as string) || ''
  const activeForm = inputData.activeForm as string | undefined

  return (
    <div className="space-y-1">
      {subject && (
        <div className="text-[11px] font-semibold text-gray-800 dark:text-gray-200">{subject}</div>
      )}
      {activeForm && (
        <div className="text-[10px] italic text-gray-500 dark:text-gray-400">{activeForm}</div>
      )}
      {description && <InlineMarkdown text={description} />}
    </div>
  )
}

const STATUS_COLORS: Record<string, string> = {
  completed: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
  in_progress: 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
  pending: 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
  deleted: 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
}

function TaskUpdateRenderer({ inputData }: ToolRendererProps) {
  const taskId = inputData.taskId as string | undefined
  const status = inputData.status as string | undefined
  const subject = inputData.subject as string | undefined
  const description = inputData.description as string | undefined
  const owner = inputData.owner as string | undefined
  const activeForm = inputData.activeForm as string | undefined

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-1.5 flex-wrap">
        {taskId && (
          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300">
            #{taskId}
          </span>
        )}
        {status && (
          <span className={cn(
            'inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono',
            STATUS_COLORS[status] || STATUS_COLORS.pending,
          )}>
            {status}
          </span>
        )}
        {owner && <Chip label="owner" value={owner} />}
        {subject && <Chip label="subject" value={subject} />}
        {activeForm && <Chip label="activeForm" value={activeForm} />}
      </div>
      {description && <InlineMarkdown text={description} />}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Web Renderers
// ---------------------------------------------------------------------------

function WebFetchRenderer({ inputData }: ToolRendererProps) {
  const url = (inputData.url as string) || ''
  const prompt = (inputData.prompt as string) || ''

  return (
    <div className="space-y-1">
      {url && (
        <div className="flex items-center gap-1.5">
          <Link className="w-3 h-3 text-blue-500 dark:text-blue-400 flex-shrink-0" />
          <a
            href={url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-[11px] font-mono text-blue-600 dark:text-blue-400 hover:underline cursor-pointer transition-colors duration-200 truncate"
          >
            {url}
          </a>
        </div>
      )}
      {prompt && <InlineMarkdown text={prompt} />}
    </div>
  )
}

function WebSearchRenderer({ inputData }: ToolRendererProps) {
  const query = (inputData.query as string) || ''
  const allowedDomains = inputData.allowed_domains as string[] | undefined
  const blockedDomains = inputData.blocked_domains as string[] | undefined

  return (
    <div className="space-y-1">
      {query && (
        <div className="flex items-center gap-1.5 px-2 py-1 rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50">
          <Search className="w-3 h-3 text-gray-400 dark:text-gray-500 flex-shrink-0" />
          <span className="text-[11px] text-gray-800 dark:text-gray-200">{query}</span>
        </div>
      )}
      <div className="flex items-center gap-1.5 flex-wrap">
        {allowedDomains?.map((d) => (
          <Chip key={d} label="allow" value={d} className="bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400" />
        ))}
        {blockedDomains?.map((d) => (
          <Chip key={d} label="block" value={d} className="bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400" />
        ))}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Notebook Renderers
// ---------------------------------------------------------------------------

function NotebookEditRenderer({ inputData, blockIdPrefix: p = '' }: ToolRendererProps) {
  const notebookPath = (inputData.notebook_path as string) || ''
  const cellNumber = inputData.cell_number as number | undefined
  const cellId = inputData.cell_id as string | undefined
  const newSource = (inputData.new_source as string) || ''
  const editMode = inputData.edit_mode as string | undefined
  const cellType = inputData.cell_type as string | undefined

  const lang = cellType === 'markdown' ? 'markdown' : 'python'

  return (
    <div className="space-y-1">
      {notebookPath && <FilePathHeader path={notebookPath} />}
      <div className="flex items-center gap-1.5 flex-wrap">
        {cellNumber != null && (
          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300">
            cell #{cellNumber}
          </span>
        )}
        {cellId && (
          <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300">
            {cellId}
          </span>
        )}
        {editMode && <Chip label="mode" value={editMode} />}
        {cellType && <Chip label="type" value={cellType} />}
      </div>
      {newSource && (
        <CompactCodeBlock code={newSource} language={lang} blockId={`${p}notebook-${notebookPath}-${cellId ?? cellNumber ?? ''}`} />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Smart MCP Renderer (fallback for mcp__* tools)
// ---------------------------------------------------------------------------

function SmartMcpRenderer({ inputData, blockIdPrefix: p = '' }: ToolRendererProps) {
  return (
    <div className="space-y-1">
      {Object.entries(inputData).map(([key, value]) => {
        // Skip null/undefined values
        if (value === null || value === undefined) return null

        const strVal = typeof value === 'string' ? value : ''

        // URL detection
        if (typeof value === 'string' && /^https?:\/\//.test(value)) {
          return (
            <div key={key} className="flex items-center gap-1.5">
              <Link className="w-3 h-3 text-blue-500 dark:text-blue-400 flex-shrink-0" />
              <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{key}:</span>
              <a
                href={value}
                target="_blank"
                rel="noopener noreferrer"
                className="text-[11px] font-mono text-blue-600 dark:text-blue-400 hover:underline cursor-pointer truncate"
              >
                {value}
              </a>
            </div>
          )
        }

        // File path detection (starts with / or ./ or ~/ or contains path separators + extension)
        if (typeof value === 'string' && /^[~./]/.test(value) && /[/\\]/.test(value)) {
          return <div key={key}><FilePathHeader path={value} /></div>
        }

        // Selector keys (uid, selector, xpath, css) — amber highlighted monospace chip
        if (['uid', 'selector', 'xpath', 'css'].includes(key) && typeof value === 'string') {
          return (
            <div key={key} className="flex items-center gap-1.5">
              <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{key}:</span>
              <span className="text-[11px] font-mono bg-amber-500/10 dark:bg-amber-500/20 rounded px-1.5 py-0.5 text-amber-800 dark:text-amber-200">
                {strVal}
              </span>
            </div>
          )
        }

        // Multi-line string (>1 newline) -> CompactCodeBlock
        if (typeof value === 'string' && (value.match(/\n/g) || []).length > 1) {
          return (
            <div key={key} className="space-y-0.5">
              <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{key}:</span>
              <CompactCodeBlock code={value} language="text" blockId={`${p}mcp-${key}`} />
            </div>
          )
        }

        // Boolean -> colored chip
        if (typeof value === 'boolean') {
          return (
            <Chip
              key={key}
              label={key}
              value={value ? 'true' : 'false'}
              className={value
                ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400'
                : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400'}
            />
          )
        }

        // Number
        if (typeof value === 'number') {
          return <Chip key={key} label={key} value={value} />
        }

        // Object or array -> CompactCodeBlock with JSON
        if (typeof value === 'object') {
          return (
            <div key={key} className="space-y-0.5">
              <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{key}:</span>
              <CompactCodeBlock code={JSON.stringify(value, null, 2)} language="json" blockId={`${p}mcp-${key}`} />
            </div>
          )
        }

        // Short string (default) -> Chip
        return <Chip key={key} label={key} value={String(value)} />
      })}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

const builtinRendererRegistry: Record<string, React.ComponentType<ToolRendererProps>> = {
  Edit: EditRenderer,
  Write: WriteRenderer,
  Read: ReadRenderer,
  Grep: GrepRenderer,
  Glob: GlobRenderer,
  Bash: BashRenderer,
  Task: TaskRenderer,
  Skill: SkillRenderer,
  SendMessage: SendMessageRenderer,
  TaskCreate: TaskCreateRenderer,
  TaskUpdate: TaskUpdateRenderer,
  WebFetch: WebFetchRenderer,
  WebSearch: WebSearchRenderer,
  NotebookEdit: NotebookEditRenderer,
}

/** Look up a tool renderer. Falls back to SmartMcpRenderer for mcp__* tools. */
export function getToolRenderer(name: string): React.ComponentType<ToolRendererProps> | null {
  if (builtinRendererRegistry[name]) return builtinRendererRegistry[name]
  if (name.startsWith('mcp__')) return SmartMcpRenderer
  return null
}

