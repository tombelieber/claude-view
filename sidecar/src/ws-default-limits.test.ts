import { once } from 'node:events'
import { type AddressInfo, Socket } from 'node:net'
import { expect, it } from 'vitest'
import WebSocket, { WebSocketServer } from 'ws'

it('bounds tiny message fragments by default and still accepts a normal connection', async () => {
  // Match production's default receiver limits; do not set a smaller test-only cap.
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 })
  const attacker = new Socket()
  const signal = AbortSignal.timeout(5000)
  attacker.setTimeout(2000, () => attacker.destroy(new Error('WebSocket transport timed out')))
  let normal: WebSocket | undefined
  let deadline: ReturnType<typeof setTimeout> | undefined
  const receiverError = new Promise<Error & { code?: string }>((resolve) => {
    server.on('connection', (socket) => {
      socket.on('error', resolve)
      socket.on('message', (message) => socket.send(message, { binary: false }))
    })
  })

  try {
    await once(server, 'listening', { signal })
    const { port } = server.address() as AddressInfo
    attacker.connect(port, '127.0.0.1')
    await once(attacker, 'connect', { signal })
    const handshake = new Promise<void>((resolve, reject) => {
      let response = ''
      const onData = (data: Buffer) => {
        response += data.toString()
        if (!response.includes('\r\n\r\n')) return
        attacker.off('data', onData)
        if (response.startsWith('HTTP/1.1 101 ')) resolve()
        else reject(new Error('WebSocket upgrade failed'))
      }
      attacker.on('data', onData)
      attacker.once('error', reject)
    })
    attacker.write(
      [
        'GET / HTTP/1.1',
        `Host: 127.0.0.1:${port}`,
        'Upgrade: websocket',
        'Connection: Upgrade',
        'Sec-WebSocket-Version: 13',
        'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==',
        '',
        '',
      ].join('\r\n'),
    )
    await handshake

    // 131,073 masked, one-byte fragments occupy < 1 MiB on the wire.
    // An unfinished message must be rejected before it grows without a bound.
    const fragmentCount = 131_073
    const frames = Buffer.alloc(fragmentCount * 7)
    for (let fragment = 0; fragment < fragmentCount; fragment++) {
      const offset = fragment * 7
      frames[offset] = fragment === 0 ? 0x01 : 0x00 // text, then continuations; FIN=0
      frames[offset + 1] = 0x81 // masked, one-byte payload; zero mask is valid
      frames[offset + 6] = 0x78
    }
    expect(frames.byteLength).toBeLessThan(1024 * 1024)
    attacker.write(frames)
    const error = await Promise.race([
      receiverError,
      new Promise<never>((_resolve, reject) => {
        deadline = setTimeout(() => reject(new Error('Fragment flood was not bounded')), 3000)
      }),
    ])
    expect(error.code).toBe('WS_ERR_TOO_MANY_BUFFERED_PARTS')
    expect(error.message).toContain('Too many message fragments')
    clearTimeout(deadline)
    attacker.destroy()

    normal = new WebSocket(`ws://127.0.0.1:${port}`, { handshakeTimeout: 2000 })
    normal.on('error', () => {}) // Cleanup may terminate a failed handshake.
    await once(normal, 'open', { signal })
    const reply = once(normal, 'message', { signal })
    normal.send('normal text still works')
    expect((await reply)[0].toString()).toBe('normal text still works')
  } finally {
    clearTimeout(deadline)
    attacker.destroy()
    normal?.terminate()
    for (const client of server.clients) client.terminate()
    await new Promise<void>((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()))
    })
  }
}, 10_000)
