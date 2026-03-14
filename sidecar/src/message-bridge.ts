import type { SDKUserMessage } from '@anthropic-ai/claude-agent-sdk'

/**
 * Promise-based async iterable that bridges imperative push() calls
 * to the AsyncIterable<SDKUserMessage> that V1 query() consumes.
 *
 * Single-iterator semantics: [Symbol.asyncIterator]() returns `this`.
 * push() and close() are both idempotent-safe after close().
 */
export class MessageBridge implements AsyncIterable<SDKUserMessage>, AsyncIterator<SDKUserMessage> {
  private queue: SDKUserMessage[] = []
  private waiters: ((result: IteratorResult<SDKUserMessage>) => void)[] = []
  private closed = false

  /** Queue a message for the SDK to consume. No-op if closed. */
  push(msg: SDKUserMessage): void {
    if (this.closed) return
    if (this.waiters.length > 0) {
      const resolve = this.waiters.shift()!
      resolve({ value: msg, done: false })
    } else {
      this.queue.push(msg)
    }
  }

  /** Terminate the iterator. Idempotent. Unblocks ALL pending next() calls. */
  close(): void {
    if (this.closed) return
    this.closed = true
    const done = {
      value: undefined as unknown as SDKUserMessage,
      done: true as const,
    }
    for (const resolve of this.waiters) {
      resolve(done)
    }
    this.waiters.length = 0
  }

  /** AsyncIterator protocol. Supports multiple concurrent waiters via queue. */
  next(): Promise<IteratorResult<SDKUserMessage>> {
    if (this.queue.length > 0) {
      return Promise.resolve({ value: this.queue.shift()!, done: false })
    }
    if (this.closed) {
      return Promise.resolve({
        value: undefined as unknown as SDKUserMessage,
        done: true,
      })
    }
    return new Promise((resolve) => {
      this.waiters.push(resolve)
    })
  }

  /** Single-iterator: returns this, not a new iterator. */
  [Symbol.asyncIterator](): AsyncIterator<SDKUserMessage> {
    return this
  }
}
