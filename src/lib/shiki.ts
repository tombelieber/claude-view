import { createHighlighter, type HighlighterGeneric, type BundledLanguage, type BundledTheme } from 'shiki'

let highlighter: HighlighterGeneric<BundledLanguage, BundledTheme> | null = null
let highlighterPromise: Promise<HighlighterGeneric<BundledLanguage, BundledTheme>> | null = null

const PRELOADED_LANGS: BundledLanguage[] = [
  'typescript', 'javascript', 'tsx', 'jsx',
  'python', 'rust', 'bash', 'json', 'html',
  'css', 'sql', 'go', 'java', 'ruby',
  'c', 'cpp', 'yaml', 'toml', 'markdown', 'diff',
]

export async function getHighlighter() {
  if (highlighter) return highlighter
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ['github-dark', 'github-light'],
      langs: PRELOADED_LANGS,
    }).then((hl) => {
      highlighter = hl
      return hl
    })
  }
  return highlighterPromise
}

export function getHighlighterSync() {
  return highlighter
}

/** Language alias map — normalizes common short names to Shiki grammar IDs */
export const LANGUAGE_ALIASES: Record<string, string> = {
  js: 'javascript',
  ts: 'typescript',
  py: 'python',
  rb: 'ruby',
  sh: 'bash',
  shell: 'bash',
  zsh: 'bash',
  yml: 'yaml',
  md: 'markdown',
}

export function resolveLanguage(lang: string): string {
  const lower = lang.toLowerCase()
  return LANGUAGE_ALIASES[lower] || lower
}

// Start loading immediately on import
getHighlighter()
