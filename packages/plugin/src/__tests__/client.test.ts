import { describe, expect, it } from 'bun:test'
import { ClaudeViewClient } from '../client.js'

describe('ClaudeViewClient', () => {
  it('uses default port 47892', () => {
    const client = new ClaudeViewClient()
    expect(client.baseUrl).toBe('http://localhost:47892')
  })

  it('reads CLAUDE_VIEW_PORT env var', () => {
    const client = new ClaudeViewClient(12345)
    expect(client.baseUrl).toBe('http://localhost:12345')
  })

  it('throws descriptive error when server is not running', async () => {
    const client = new ClaudeViewClient(19999)
    await expect(client.get('/api/health')).rejects.toThrow(/claude-view server not detected/)
  })
})
