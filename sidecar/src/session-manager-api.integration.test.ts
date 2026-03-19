import { describe, it } from 'vitest'

describe('SessionManager REST API', () => {
  it.todo('POST /api/sessions → 201 with { sessionId, status }')
  it.todo('GET /api/sessions → 200 with array of session info')
  it.todo('POST /api/sessions/:id/resume → 200 (valid), 404 (unknown)')
  it.todo('DELETE /api/sessions/:id → 200 (valid), 404 (unknown)')
  it.todo('GET /api/sessions/:id/status → 200 with model, mode, context, cost')
})
