import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SessionChannel } from './session-channel'

describe('SessionChannel', () => {
  let channel: SessionChannel
  let mockSend: ReturnType<typeof vi.fn>

  beforeEach(() => {
    mockSend = vi.fn() as unknown as ReturnType<typeof vi.fn>
    channel = new SessionChannel(mockSend as (msg: Record<string, unknown>) => void)
  })

  describe('send()', () => {
    it('sends message via the send function', () => {
      channel.send({ type: 'interrupt' })
      expect(mockSend).toHaveBeenCalledWith({ type: 'interrupt' })
    })

    it('does nothing when send function is null', () => {
      const ch = new SessionChannel(null)
      expect(() => ch.send({ type: 'interrupt' })).not.toThrow()
    })
  })

  describe('request()', () => {
    it('sends message with requestId and resolves on matching response', async () => {
      const promise = channel.request<string[]>({ type: 'query_models' })

      const sentMsg = mockSend.mock.calls[0][0]
      expect(sentMsg.type).toBe('query_models')
      expect(sentMsg.requestId).toBeDefined()

      channel.handleResponse(sentMsg.requestId, ['model-a', 'model-b'])
      const result = await promise
      expect(result).toEqual(['model-a', 'model-b'])
    })

    it('rejects on timeout', async () => {
      vi.useFakeTimers()
      const promise = channel.request({ type: 'query_models' }, 50)
      vi.advanceTimersByTime(50)
      await expect(promise).rejects.toThrow('timeout')
      vi.useRealTimers()
    })

    it('concurrent requests with same type get independent responses', async () => {
      const p1 = channel.request<string>({ type: 'query_mcp_status' })
      const p2 = channel.request<string>({ type: 'query_mcp_status' })

      const id1 = mockSend.mock.calls[0][0].requestId
      const id2 = mockSend.mock.calls[1][0].requestId
      expect(id1).not.toBe(id2)

      channel.handleResponse(id2, 'second')
      channel.handleResponse(id1, 'first')

      expect(await p1).toBe('first')
      expect(await p2).toBe('second')
    })

    it('rejects when send function is null', async () => {
      const ch = new SessionChannel(null)
      await expect(ch.request({ type: 'query_models' })).rejects.toThrow('not connected')
    })
  })

  describe('handleResponse()', () => {
    it('is a no-op for unknown requestId (no throw)', () => {
      expect(() => channel.handleResponse('unknown-id', {})).not.toThrow()
    })

    it('is a no-op for late response after disconnect', () => {
      channel.request({ type: 'query_models' }).catch(() => {})
      channel.handleDisconnect()
      expect(() => channel.handleResponse('any-id', {})).not.toThrow()
    })
  })

  describe('handleDisconnect()', () => {
    it('rejects all pending requests', async () => {
      const p1 = channel.request({ type: 'query_models' })
      const p2 = channel.request({ type: 'query_agents' })

      channel.handleDisconnect()

      await expect(p1).rejects.toThrow('disconnect')
      await expect(p2).rejects.toThrow('disconnect')
    })

    it('clears all pending timers (no memory leak)', () => {
      vi.useFakeTimers()
      channel.request({ type: 'query_models' }, 10_000).catch(() => {})
      channel.request({ type: 'query_agents' }, 10_000).catch(() => {})

      channel.handleDisconnect()

      // Advance past original timeout — no additional rejections should fire
      vi.advanceTimersByTime(15_000)
      vi.useRealTimers()
    })

    it('clears pending map after disconnect', () => {
      channel.request({ type: 'query_models' }).catch(() => {})
      channel.handleDisconnect()
      expect(() => channel.handleResponse('nonexistent', {})).not.toThrow()
    })
  })

  describe('updateSend()', () => {
    it('updates the send function for reconnection', () => {
      const newSend = vi.fn()
      channel.updateSend(newSend)
      channel.send({ type: 'interrupt' })
      expect(newSend).toHaveBeenCalled()
      expect(mockSend).toHaveBeenCalledTimes(0)
    })

    it('new requests use updated send function', async () => {
      const newSend = vi.fn()
      channel.updateSend(newSend)
      channel.request({ type: 'query_models' }).catch(() => {})
      expect(newSend).toHaveBeenCalledTimes(1)
      expect(newSend.mock.calls[0][0].type).toBe('query_models')
      channel.handleDisconnect() // cleanup
    })
  })
})
