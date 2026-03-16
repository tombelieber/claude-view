import { execFileSync } from 'node:child_process'

let cachedPath: string | undefined

export function findClaudeExecutable(): string {
  const envPath = process.env.CLAUDE_VIEW_CLI_PATH
  if (envPath) return envPath

  if (cachedPath) return cachedPath

  try {
    const cmd = process.platform === 'win32' ? 'where' : 'which'
    const output = execFileSync(cmd, ['claude'], {
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim()
    const resolved = output.split(/\r?\n/)[0].trim()
    if (resolved) {
      cachedPath = resolved
      return resolved
    }
  } catch {
    // which/where failed
  }

  throw new Error(
    'Claude Code CLI not found in PATH. ' +
      'Install with: npm install -g @anthropic-ai/claude-code\n' +
      'Or set CLAUDE_VIEW_CLI_PATH to the claude binary path.\n' +
      'If claude was recently installed, restart the sidecar to pick up the new PATH.',
  )
}

export function _resetCachedPathForTesting(): void {
  cachedPath = undefined
}
