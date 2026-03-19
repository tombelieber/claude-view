import { describe, it } from 'vitest'

describe('SessionManager', () => {
  it.todo('create() returns ManagedSession with status creating → active')
  it.todo('resume() throws if sessionId unknown')
  it.todo('resume() passes cwd to SDK query() options')
  it.todo('end() calls query.close(), removes from map, status → ended')
  it.todo('list() returns all sessions with correct info')
  it.todo('takeover() sends SIGTERM, waits, SIGKILL after 3s, then resumes')
  it.todo('wsClients Set allows multiple browser tabs')
})
