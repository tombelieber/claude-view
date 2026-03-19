// sidecar/src/session-manager.ts
// SessionManager wraps the session lifecycle functions from sdk-session.ts.
// Single responsibility: coordinate create/resume/end lifecycle on top of
// the registry, hiding the raw SDK wiring from callers.

import type { CreateSessionRequest, ResumeSessionRequest } from './protocol.js'
import {
  closeSession,
  createControlSession,
  resumeControlSession,
  waitForSessionInit,
} from './sdk-session.js'
import type { ActiveSession, ControlSession, SessionRegistry } from './session-registry.js'

export class SessionManager {
  constructor(private registry: SessionRegistry) {}

  async create(req: CreateSessionRequest): Promise<ControlSession> {
    const cs = createControlSession(req, this.registry)
    await waitForSessionInit(cs, 15_000)
    return cs
  }

  async resume(
    sessionId: string,
    opts: Omit<ResumeSessionRequest, 'sessionId'>,
  ): Promise<ControlSession> {
    const cs = await resumeControlSession({ sessionId, ...opts }, this.registry)
    await waitForSessionInit(cs, 15_000)
    return cs
  }

  async end(sessionId: string): Promise<void> {
    const cs = this.registry.getBySessionId(sessionId)
    if (!cs) throw new Error(`Session ${sessionId} not found`)
    closeSession(cs, this.registry)
  }

  get(sessionId: string): ControlSession | undefined {
    return this.registry.getBySessionId(sessionId)
  }

  list(): ActiveSession[] {
    return this.registry.list()
  }
}
