// --- JSON Detection ---

/** Try to parse a string as JSON. Returns parsed value or null. */
export function tryParseJson(str: string): unknown {
  try {
    const trimmed = str.trim()
    if ((!trimmed.startsWith('{') && !trimmed.startsWith('[')) || trimmed.length < 2) return null
    return JSON.parse(trimmed)
  } catch {
    return null
  }
}

/** Check if a string is valid JSON object or array. */
export function isJsonContent(content: string): boolean {
  return tryParseJson(content) !== null
}

/** Heuristic: content looks like a unified diff (git diff, patch output).
 *  Checks for diff headers or a high density of +/- prefixed lines. */
export function isDiffContent(content: string): boolean {
  // Quick checks for diff headers
  if (content.startsWith('diff --git') || content.startsWith('---') || content.startsWith('Index:')) return true
  const lines = content.split('\n')
  if (lines.length < 3) return false
  const nonEmpty = lines.filter((l) => l.length > 0)
  if (nonEmpty.length < 3) return false
  // Count lines that look like diff hunks: @@, +line, -line
  const diffLines = nonEmpty.filter((l) => /^[+-][^+-]/.test(l) || l.startsWith('@@')).length
  return diffLines / nonEmpty.length >= 0.3
}

/** Heuristic: content looks like file output (Read tool, grep, etc.).
 *  Matches lines starting with optional whitespace + digits + → (cat -n format),
 *  or lines starting with filepath:line patterns like "src/foo.tsx:42:". */
const LINE_NUM_RE = /^\s*\d+[→\t|:]/
export function isCodeLikeContent(content: string): boolean {
  const lines = content.split('\n')
  if (lines.length < 2) return false
  // If >=40% of non-empty lines match the pattern, treat as code
  const nonEmpty = lines.filter((l) => l.trim().length > 0)
  if (nonEmpty.length < 2) return false
  const matching = nonEmpty.filter((l) => LINE_NUM_RE.test(l)).length
  return matching / nonEmpty.length >= 0.4
}

/** Strip line-number prefixes (e.g. "  42-> ") so heuristics see actual code. */
export function stripLineNumbers(content: string): string {
  return content.replace(/^\s*\d+[→\t|]\s?/gm, '')
}

/** Detect the best Shiki language hint for code-like content. */
export function detectCodeLanguage(content: string): string {
  if (isDiffContent(content)) return 'diff'
  const raw = stripLineNumbers(content)
  // Rust
  if (/\b(use\s+(std|crate|super|self)::|fn\s+\w+\s*[<(]|impl\s+(<.*>)?\s*\w+|pub\s+(fn|struct|enum|mod|type|trait|const|static)\b|let\s+mut\b|#\[derive)/.test(raw)) return 'rust'
  // TypeScript / TSX / JS
  if (/\bimport\s+.*\bfrom\s+['"]/.test(raw) || /\bexport\s+(default\s+)?(function|const|class|interface|type)\b/.test(raw) || (/\bconst\s+\w+\s*[:=]/.test(raw) && /\b(string|number|boolean|Promise|async|await)\b/.test(raw))) {
    return /\bReact\b|['"]react['"]|<\w+[A-Z]|className=|useState|useEffect/.test(raw) ? 'tsx' : 'typescript'
  }
  // Python
  if (/\b(def\s+\w+\s*\(|class\s+\w+[\s:(]|from\s+\w+\s+import|import\s+\w+|self\.\w+|__\w+__|@\w+)/.test(raw)) return 'python'
  // Go
  if (/\bpackage\s+\w+/.test(raw) && /\bfunc\s+/.test(raw)) return 'go'
  // SQL
  if (/\b(SELECT|INSERT INTO|CREATE TABLE|ALTER TABLE|UPDATE\s+\w+\s+SET|DELETE FROM)\b/i.test(raw)) return 'sql'
  // HTML / JSX (angle brackets with component names)
  if (/<\/?[A-Z]\w+[\s/>]/.test(raw) || /<(div|span|section|header|footer|main|form|button|input)\b/i.test(raw)) return 'tsx'
  // CSS
  if (/[.#]\w+\s*\{[\s\S]*?[;}]/.test(raw) && /\b(color|background|margin|padding|display|flex|grid)\s*:/.test(raw)) return 'css'
  // Bash / shell
  if (/^#!\//.test(raw) || /\b(echo|export|source|chmod|mkdir|cd|ls|grep|sed|awk|curl|wget)\b/.test(raw)) return 'bash'
  // JSON (didn't pass isJsonContent but has JSON-like structure)
  if (/^\s*["{\[]/.test(raw.trim())) return 'json'
  // YAML / TOML
  if (/^\w[\w-]*\s*[:=]\s/.test(raw) && !/[;{}()]/.test(raw.slice(0, 200))) return 'yaml'
  // C / C++
  if (/\b(#include\s*<|int\s+main\s*\(|void\s+\w+\(|printf\s*\()/.test(raw)) return 'c'
  // Java
  if (/\b(public\s+class|private\s+|protected\s+|System\.out)/.test(raw)) return 'java'
  // Ruby
  if (/\b(require\s+['"]|module\s+\w+|class\s+\w+\s*<|end\b.*\n.*\bdef\b)/.test(raw)) return 'ruby'
  // Markdown
  if (/^#{1,6}\s+\w/.test(raw) && /\n#{1,6}\s+\w/.test(raw)) return 'markdown'
  return 'text'
}

// --- Tool Name Helpers ---

/** Shorten verbose MCP tool names: "mcp__chrome-devtools__take_snapshot" -> "take_snapshot"
 *  Also strips common prefixes for readability in the terminal view. */
export function shortenToolName(name: string): { short: string; server?: string } {
  // MCP tools: mcp__<server>__<tool_name>
  const mcpMatch = /^mcp__([^_]+(?:_[^_]+)*)__(.+)$/.exec(name)
  if (mcpMatch) {
    return { short: mcpMatch[2], server: mcpMatch[1] }
  }
  return { short: name }
}

/** Pick a distinct color class based on tool category. */
export function toolChipColor(name: string): string {
  // MCP tools
  if (name.startsWith('mcp__')) return 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300'
  // Task / sub-agent
  if (name === 'Task') return 'bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300'
  // Skill
  if (name === 'Skill') return 'bg-purple-500/10 dark:bg-purple-500/20 text-purple-700 dark:text-purple-300'
  // Common built-in tools
  if (name === 'Read' || name === 'Glob' || name === 'Grep') return 'bg-emerald-500/10 dark:bg-emerald-500/20 text-emerald-700 dark:text-emerald-300'
  if (name === 'Write' || name === 'Edit') return 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300'
  if (name === 'Bash') return 'bg-gray-500/10 dark:bg-gray-500/20 text-gray-700 dark:text-gray-300'
  // Default (orange for other tools)
  return 'bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300'
}
