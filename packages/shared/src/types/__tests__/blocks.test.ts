import { describe, expect, it } from 'vitest'
import { type ConversationBlock, type ProgressBlock, isProgressBlock } from '../blocks'

describe('isProgressBlock', () => {
  it('returns true for ProgressBlock', () => {
    const block: ProgressBlock = {
      type: 'progress',
      id: 'p-1',
      variant: 'bash',
      category: 'builtin',
      data: {
        type: 'bash',
        output: '',
        fullOutput: '',
        elapsedTimeSeconds: 0,
        totalLines: 0,
        totalBytes: BigInt(0),
      },
      ts: 123456,
    }
    expect(isProgressBlock(block)).toBe(true)
  })

  it('returns false for non-ProgressBlock', () => {
    const block: ConversationBlock = {
      type: 'user',
      id: 'u-1',
      text: 'hello',
      timestamp: 123456,
    }
    expect(isProgressBlock(block)).toBe(false)
  })
})
