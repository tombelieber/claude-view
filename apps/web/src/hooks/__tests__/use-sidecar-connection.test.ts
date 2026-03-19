import { describe, it } from 'vitest'

describe('useSidecarConnection', () => {
  it.todo('connects to /ws/chat/:sessionId on mount')
  it.todo('sets isLive=true when WS opens')
  it.todo('sets isLive=false when WS closes')
  it.todo('updates committedBlocks on blocks_snapshot message')
  it.todo('updates committedBlocks on blocks_update message')
  it.todo('clears pendingText on blocks_update')
  it.todo('reconnects after WS close with backoff')
  it.todo('receives fresh blocks_snapshot on reconnect — no duplicate blocks')
  it.todo('status transitions: active → idle → error follow session_state events')
  it.todo('does not connect when skip=true (watching mode)')
})
