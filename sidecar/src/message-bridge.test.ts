import { describe, expect, it } from 'vitest'
import { MessageBridge } from './message-bridge.js'

describe('MessageBridge', () => {
  it('yields pushed messages in order', async () => {
    const bridge = new MessageBridge()
    const msg1 = makeFakeUserMessage('hello')
    const msg2 = makeFakeUserMessage('world')
    bridge.push(msg1)
    bridge.push(msg2)

    const iter = bridge[Symbol.asyncIterator]()
    const r1 = await iter.next()
    const r2 = await iter.next()
    expect(r1.value).toBe(msg1)
    expect(r2.value).toBe(msg2)
    expect(r1.done).toBe(false)
    expect(r2.done).toBe(false)
  })

  it('blocks next() until push() is called', async () => {
    const bridge = new MessageBridge()
    const iter = bridge[Symbol.asyncIterator]()

    let resolved = false
    const promise = iter.next().then((r) => {
      resolved = true
      return r
    })

    await new Promise((r) => setTimeout(r, 50))
    expect(resolved).toBe(false)

    const msg = makeFakeUserMessage('delayed')
    bridge.push(msg)
    const result = await promise
    expect(result.value).toBe(msg)
    expect(resolved).toBe(true)
  })

  it('close() unblocks pending next() with done: true', async () => {
    const bridge = new MessageBridge()
    const iter = bridge[Symbol.asyncIterator]()

    const promise = iter.next()
    bridge.close()
    const result = await promise
    expect(result.done).toBe(true)
  })

  it('next() after close() returns done: true immediately', async () => {
    const bridge = new MessageBridge()
    bridge.close()
    const iter = bridge[Symbol.asyncIterator]()
    const result = await iter.next()
    expect(result.done).toBe(true)
  })

  it('push() after close() is a silent no-op', () => {
    const bridge = new MessageBridge()
    bridge.close()
    bridge.push(makeFakeUserMessage('dropped'))
  })

  it('close() is idempotent', () => {
    const bridge = new MessageBridge()
    bridge.close()
    bridge.close()
  })

  it('returns this from [Symbol.asyncIterator] (single-iterator)', () => {
    const bridge = new MessageBridge()
    const iter1 = bridge[Symbol.asyncIterator]()
    const iter2 = bridge[Symbol.asyncIterator]()
    expect(iter1).toBe(iter2)
    expect(iter1).toBe(bridge)
  })

  it('drains queued messages before blocking', async () => {
    const bridge = new MessageBridge()
    bridge.push(makeFakeUserMessage('a'))
    bridge.push(makeFakeUserMessage('b'))
    bridge.push(makeFakeUserMessage('c'))
    bridge.close()

    const results: string[] = []
    for await (const msg of bridge) {
      const text = (msg.message as { content: { text: string }[] }).content[0].text
      results.push(text)
    }
    expect(results).toEqual(['a', 'b', 'c'])
  })

  it('supports multiple concurrent waiters via queue', async () => {
    const bridge = new MessageBridge()
    const p1 = bridge.next()
    const p2 = bridge.next()
    bridge.push(makeFakeUserMessage('first'))
    bridge.push(makeFakeUserMessage('second'))
    const r1 = await p1
    const r2 = await p2
    expect(r1.done).toBe(false)
    expect(r2.done).toBe(false)
    expect((r1.value.message as any).content[0].text).toBe('first')
    expect((r2.value.message as any).content[0].text).toBe('second')
  })

  it('repeated next() after done always returns done', async () => {
    const bridge = new MessageBridge()
    bridge.close()
    const r1 = await bridge.next()
    const r2 = await bridge.next()
    const r3 = await bridge.next()
    expect(r1.done).toBe(true)
    expect(r2.done).toBe(true)
    expect(r3.done).toBe(true)
  })
})

function makeFakeUserMessage(text: string) {
  return {
    type: 'user' as const,
    session_id: '',
    message: {
      role: 'user' as const,
      content: [{ type: 'text' as const, text }],
    },
    parent_tool_use_id: null,
  }
}
