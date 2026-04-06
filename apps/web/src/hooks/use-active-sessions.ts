import { useQuery } from '@tanstack/react-query'

// ── Types matching Rust ActiveSession ──

export interface ActiveSession {
  pid: number
  sessionId: string
  cwd: string
  startedAt: number
  kind: string // "interactive" | "background"
  entrypoint: string // "cli" | "claude-vscode" | "claude-desktop" | "claude-web"
}

// ── Fetch ──

async function fetchActiveSessions(): Promise<ActiveSession[]> {
  const res = await fetch('/api/active-sessions')
  if (!res.ok) throw new Error('Failed to fetch active sessions')
  return res.json()
}

// ── Hook ──

/** Fetch active Claude Code sessions from ~/.claude/sessions/. */
export function useActiveSessions() {
  return useQuery({
    queryKey: ['active-sessions'],
    queryFn: fetchActiveSessions,
    staleTime: 5_000, // sessions are ephemeral, refresh frequently
  })
}
