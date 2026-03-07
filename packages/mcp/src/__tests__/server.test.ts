import { describe, expect, it } from 'bun:test'
import { TOOL_COUNT, createServer } from '../server.js'

describe('createServer', () => {
  it('creates an MCP server instance', () => {
    const server = createServer()
    expect(server).toBeDefined()
  })

  it('exports correct tool count', () => {
    expect(TOOL_COUNT).toBe(8)
  })
})
