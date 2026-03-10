import { describe, expect, it } from 'vitest'
import { parseRichMessage } from './RichPane'

describe('parseRichMessage', () => {
  describe('system messages', () => {
    it('extracts metadata for system messages', () => {
      const raw = JSON.stringify({
        type: 'system',
        content: 'queue-enqueue: fix the bug',
        category: 'queue',
        ts: '2026-03-09T10:00:00Z',
        metadata: {
          type: 'queue-operation',
          operation: 'enqueue',
          content: 'fix the bug',
        },
      })

      const result = parseRichMessage(raw)

      expect(result).not.toBeNull()
      expect(result!.type).toBe('system')
      expect(result!.metadata).toEqual({
        type: 'queue-operation',
        operation: 'enqueue',
        content: 'fix the bug',
      })
    })

    it('handles system messages without metadata', () => {
      const raw = JSON.stringify({
        type: 'system',
        content: 'some system event',
        category: 'system',
      })

      const result = parseRichMessage(raw)

      expect(result).not.toBeNull()
      expect(result!.metadata).toBeUndefined()
    })
  })
})
