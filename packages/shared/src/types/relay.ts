/** Session status as reported by the Mac daemon */
export type SessionStatus = 'active' | 'waiting' | 'idle' | 'done'

/** Mac → Phone: session snapshot */
export interface RelaySessionSnapshot {
  type: 'sessions'
  sessions: RelaySession[]
}

export interface RelaySession {
  id: string
  project: string
  model: string
  status: SessionStatus
  cost_usd: number
  tokens: { input: number; output: number }
  last_message: string
  updated_at: number
}

/** Mac → Phone: live output stream */
export interface RelayOutputStream {
  type: 'output'
  session_id: string
  chunks: RelayOutputChunk[]
}

export interface RelayOutputChunk {
  role: 'assistant' | 'tool' | 'user'
  text?: string
  name?: string
  path?: string
}

/** Phone → Mac: command */
export interface RelayCommand {
  type: 'command'
  action: string
  session_id?: string
  [key: string]: unknown
}

/** Union of all relay message types */
export type RelayMessage = RelaySessionSnapshot | RelayOutputStream | RelayCommand
