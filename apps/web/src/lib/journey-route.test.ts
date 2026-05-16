import { describe, expect, it } from 'vitest'
import { surfaceForPath } from './journey-route'

describe('surfaceForPath', () => {
  it('maps every real top-level route to a Surface', () => {
    expect(surfaceForPath('/')).toBe('live_monitor')
    expect(surfaceForPath('/chat')).toBe('chat')
    expect(surfaceForPath('/chat/abc-123')).toBe('chat')
    expect(surfaceForPath('/sessions')).toBe('history')
    expect(surfaceForPath('/sessions/sess_42')).toBe('session_detail')
    expect(surfaceForPath('/analytics')).toBe('analytics')
    expect(surfaceForPath('/activity')).toBe('activity')
    expect(surfaceForPath('/reports')).toBe('reports')
    expect(surfaceForPath('/prompts')).toBe('prompts')
    expect(surfaceForPath('/teams')).toBe('teams')
    expect(surfaceForPath('/workflows')).toBe('workflows')
    expect(surfaceForPath('/workflows/wf-1')).toBe('workflows')
    expect(surfaceForPath('/plugins')).toBe('plugins')
    expect(surfaceForPath('/memory')).toBe('memory')
    expect(surfaceForPath('/monitor')).toBe('system_monitor')
    expect(surfaceForPath('/settings')).toBe('settings')
    expect(surfaceForPath('/search')).toBe('search')
    expect(surfaceForPath('/insights')).toBe('insights')
  })

  it('normalizes trailing slashes', () => {
    expect(surfaceForPath('/sessions/')).toBe('history')
    expect(surfaceForPath('/settings//')).toBe('settings')
  })

  it('returns null for unknown routes (emit nothing, never a raw path)', () => {
    expect(surfaceForPath('/totally-unknown')).toBeNull()
    expect(surfaceForPath('/sessions/abc/secret/extra')).toBe('session_detail')
    expect(surfaceForPath('/admin/secret-path')).toBeNull()
  })
})
