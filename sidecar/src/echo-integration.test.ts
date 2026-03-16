// Echo wire format integration tests — verifies the full path from
// emitSequenced → RingBuffer → getAfter retrieval.
import { EventEmitter } from 'node:events'
import { describe, expect, it } from 'vitest'
import type { SequencedEvent, ServerEvent } from './protocol.js'
import { RingBuffer } from './ring-buffer.js'

// Inline emitSequenced logic (mirrors SessionRegistry.emitSequenced)
// to test the wire format contract without heavy ControlSession dependencies.
function emitSequenced(
  eventBuffer: RingBuffer<{ seq: number; msg: SequencedEvent }>,
  emitter: EventEmitter,
  nextSeq: { value: number },
  event: ServerEvent,
): SequencedEvent {
  const seq = nextSeq.value++
  const sequenced: SequencedEvent = { ...event, seq }
  eventBuffer.push({ seq, msg: sequenced })
  emitter.emit('message', sequenced)
  return sequenced
}

describe('Echo wire format integration (emitSequenced + RingBuffer)', () => {
  it('emitSequenced wraps user_message_echo with seq number in ring buffer', () => {
    const buffer = new RingBuffer<{ seq: number; msg: SequencedEvent }>(100)
    const emitter = new EventEmitter()
    const nextSeq = { value: 0 }

    const echo: ServerEvent = {
      type: 'user_message_echo',
      content: 'Hello from user',
      timestamp: 1710000000,
    } as ServerEvent

    emitSequenced(buffer, emitter, nextSeq, echo)

    // Verify buffer has 1 entry with seq=0
    const items = buffer.toArray()
    expect(items).toHaveLength(1)
    expect(items[0].seq).toBe(0)
    expect(items[0].msg.type).toBe('user_message_echo')
    expect(items[0].msg.seq).toBe(0)
    expect((items[0].msg as unknown as { content: string }).content).toBe('Hello from user')
  })

  it('echo at seq 0 is retrievable via getAfter(-1)', () => {
    const buffer = new RingBuffer<{ seq: number; msg: SequencedEvent }>(100)
    const emitter = new EventEmitter()
    const nextSeq = { value: 0 }

    const echo: ServerEvent = {
      type: 'user_message_echo',
      content: 'First message',
      timestamp: 1710000000,
    } as ServerEvent

    emitSequenced(buffer, emitter, nextSeq, echo)

    // getAfter(-1) returns all events — this is what the WS handler uses on
    // first connect to replay the full buffer for newly connected clients.
    const replayed = buffer.getAfter(-1, (item) => item.seq)
    expect(replayed).not.toBeNull()
    expect(replayed).toHaveLength(1)
    expect(replayed![0].msg.type).toBe('user_message_echo')
    expect(replayed![0].msg.seq).toBe(0)
  })

  it('echo followed by assistant events maintains insertion order', () => {
    const buffer = new RingBuffer<{ seq: number; msg: SequencedEvent }>(100)
    const emitter = new EventEmitter()
    const nextSeq = { value: 0 }

    // Simulate: user echo → assistant text → turn complete
    emitSequenced(buffer, emitter, nextSeq, {
      type: 'user_message_echo',
      content: 'Question',
      timestamp: 1710000000,
    } as ServerEvent)

    emitSequenced(buffer, emitter, nextSeq, {
      type: 'assistant_text',
      text: 'Answer',
      messageId: 'a1',
      parentToolUseId: null,
    } as ServerEvent)

    emitSequenced(buffer, emitter, nextSeq, {
      type: 'turn_complete',
      totalCostUsd: 0.01,
      numTurns: 1,
      durationMs: 500,
      durationApiMs: 400,
      usage: {},
      modelUsage: {},
      permissionDenials: [],
      result: 'stop',
      stopReason: 'end_turn',
    } as ServerEvent)

    const all = buffer.getAfter(-1, (item) => item.seq)
    expect(all).toHaveLength(3)
    expect(all![0].msg.type).toBe('user_message_echo')
    expect(all![0].msg.seq).toBe(0)
    expect(all![1].msg.type).toBe('assistant_text')
    expect(all![1].msg.seq).toBe(1)
    expect(all![2].msg.type).toBe('turn_complete')
    expect(all![2].msg.seq).toBe(2)
  })

  it('emitter receives sequenced event synchronously', () => {
    const buffer = new RingBuffer<{ seq: number; msg: SequencedEvent }>(100)
    const emitter = new EventEmitter()
    const nextSeq = { value: 0 }
    const received: SequencedEvent[] = []

    emitter.on('message', (msg: SequencedEvent) => received.push(msg))

    emitSequenced(buffer, emitter, nextSeq, {
      type: 'user_message_echo',
      content: 'live event',
      timestamp: 1710000000,
    } as ServerEvent)

    // Emitter fires synchronously — message available immediately
    expect(received).toHaveLength(1)
    expect(received[0].type).toBe('user_message_echo')
    expect(received[0].seq).toBe(0)
  })

  it('getAfter(N) skips echo events with seq <= N (reconnect scenario)', () => {
    const buffer = new RingBuffer<{ seq: number; msg: SequencedEvent }>(100)
    const emitter = new EventEmitter()
    const nextSeq = { value: 0 }

    // Fill buffer with 3 events (seq 0, 1, 2)
    emitSequenced(buffer, emitter, nextSeq, {
      type: 'user_message_echo',
      content: 'msg 1',
      timestamp: 1710000000,
    } as ServerEvent)

    emitSequenced(buffer, emitter, nextSeq, {
      type: 'assistant_text',
      text: 'reply',
      messageId: 'a1',
      parentToolUseId: null,
    } as ServerEvent)

    emitSequenced(buffer, emitter, nextSeq, {
      type: 'user_message_echo',
      content: 'msg 2',
      timestamp: 1710000001,
    } as ServerEvent)

    // Client reconnects having seen up to seq 0 — should get events 1 and 2
    const missed = buffer.getAfter(0, (item) => item.seq)
    expect(missed).toHaveLength(2)
    expect(missed![0].msg.seq).toBe(1)
    expect(missed![1].msg.seq).toBe(2)
    expect(missed![1].msg.type).toBe('user_message_echo')
  })
})
