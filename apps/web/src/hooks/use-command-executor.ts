import type { ActiveSession } from '@claude-view/shared/types/sidecar-protocol'
import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import type { ChatPanelStore, Command, RawEvent } from '../lib/chat-panel'
import { mapWsEvent } from '../lib/chat-panel'
import { fetchInitialHistory } from '../lib/fetch-initial-history'
import { SessionChannel } from '../lib/session-channel'
import { sidecarWsUrl } from '../lib/ws-url'
import { wsUrl } from '../lib/ws-url'
import { NON_RECOVERABLE_CODES } from '../types/control'

const HEARTBEAT_INTERVAL_MS = 15_000

export function useCommandExecutor(
  store: ChatPanelStore,
  dispatch: (event: RawEvent) => void,
  pendingCmdsRef: React.MutableRefObject<Command[]>,
) {
  const wsRef = useRef<WebSocket | null>(null)
  const terminalWsRef = useRef<WebSocket | null>(null)
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())
  const heartbeatRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const pongReceivedRef = useRef(true)
  const channelRef = useRef(new SessionChannel(null))
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  // Drain pending commands after each render
  useEffect(() => {
    const cmds = pendingCmdsRef.current.splice(0)
    for (const cmd of cmds) {
      executeCommand(cmd)
    }
  })

  function executeCommand(cmd: Command) {
    switch (cmd.cmd) {
      case 'FETCH_HISTORY': {
        fetchInitialHistory(cmd.sessionId)
          .then((result) => dispatch(result))
          .catch((err) => dispatch({ type: 'HISTORY_FAILED', error: err.message }))
        break
      }
      case 'FETCH_OLDER_HISTORY': {
        const params = new URLSearchParams({
          limit: String(cmd.limit),
          offset: String(cmd.offset),
          format: 'block',
        })
        fetch(`/api/sessions/${encodeURIComponent(cmd.sessionId)}/messages?${params}`)
          .then(async (r) => {
            if (!r.ok) throw new Error(`Failed to fetch older history (${r.status})`)
            return r.json()
          })
          .then((data) =>
            dispatch({
              type: 'OLDER_HISTORY_OK',
              blocks: data.blocks ?? [],
              offset: cmd.offset,
            }),
          )
          .catch(() => {
            // On error, reset fetchingOlder flag by dispatching a no-op older history
            dispatch({ type: 'OLDER_HISTORY_OK', blocks: [], offset: cmd.offset })
          })
        break
      }
      case 'CHECK_SIDECAR_ACTIVE': {
        // Only match sessions in healthy states — zombies (closed/error/initializing)
        // would trigger SIDECAR_HAS_SESSION but have no cached session_init,
        // causing the 10s init timeout on WS connect.
        const HEALTHY_STATES = new Set([
          'waiting_input',
          'active',
          'waiting_permission',
          'compacting',
        ])
        fetch('/api/sidecar/sessions')
          .then((r) => r.json())
          .then((data: { active: ActiveSession[] }) => {
            const active = data.active.find(
              (s) => s.sessionId === cmd.sessionId && HEALTHY_STATES.has(s.state),
            )
            if (active) {
              dispatch({
                type: 'SIDECAR_HAS_SESSION',
                controlId: active.controlId,
                sessionState: active.state,
              })
            } else {
              dispatch({ type: 'SIDECAR_NO_SESSION' })
            }
          })
          .catch(() => dispatch({ type: 'SIDECAR_NO_SESSION' }))
        break
      }
      case 'POST_CREATE': {
        fetch('/api/sidecar/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            model: cmd.model,
            initialMessage: cmd.message,
            permissionMode: cmd.permissionMode,
            persistSession: cmd.persistSession,
            projectPath: cmd.projectPath,
          }),
        })
          .then(async (r) => {
            const data = await r.json()
            if (!r.ok) throw new Error(data.error || `Create failed (${r.status})`)
            return data
          })
          .then((data) =>
            dispatch({
              type: 'ACQUIRE_OK',
              controlId: data.controlId,
              sessionId: data.sessionId,
            }),
          )
          .catch((err) => dispatch({ type: 'ACQUIRE_FAILED', error: err.message }))
        break
      }
      case 'POST_RESUME': {
        fetch(`/api/sidecar/sessions/${encodeURIComponent(cmd.sessionId)}/resume`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            permissionMode: cmd.permissionMode,
            model: cmd.model,
            resumeAtMessageId: cmd.resumeAtMessageId,
            projectPath: cmd.projectPath,
            initialMessage: cmd.message,
          }),
        })
          .then(async (r) => {
            const data = await r.json()
            if (!r.ok) throw new Error(data.error || `Resume failed (${r.status})`)
            return data
          })
          .then((data) => dispatch({ type: 'ACQUIRE_OK', controlId: data.controlId }))
          .catch((err) => dispatch({ type: 'ACQUIRE_FAILED', error: err.message }))
        break
      }
      case 'POST_FORK': {
        fetch(`/api/sidecar/sessions/${encodeURIComponent(cmd.sessionId)}/fork`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ message: cmd.message, projectPath: cmd.projectPath }),
        })
          .then((r) => r.json())
          .then((data) =>
            dispatch({
              type: 'ACQUIRE_OK',
              controlId: data.controlId,
              sessionId: data.sessionId,
            }),
          )
          .catch((err) => dispatch({ type: 'ACQUIRE_FAILED', error: err.message }))
        break
      }
      case 'OPEN_SIDECAR_WS': {
        // Close existing WS if any
        if (wsRef.current) {
          wsRef.current.onclose = null
          wsRef.current.close()
        }
        const ws = new WebSocket(sidecarWsUrl(`/ws/chat/${cmd.sessionId}`))
        wsRef.current = ws

        ws.onopen = () => {
          dispatch({ type: 'WS_OPEN' })
          startHeartbeat(ws)
          channelRef.current.updateSend((msg) => ws.send(JSON.stringify(msg)))
        }

        ws.onmessage = (ev) => {
          let raw: Record<string, unknown>
          try {
            raw = JSON.parse(ev.data)
          } catch {
            return
          }

          // Heartbeat infrastructure — NOT FSM
          if (raw.type === 'pong') {
            pongReceivedRef.current = true
            return
          }
          if (raw.type === 'heartbeat_config') {
            startHeartbeat(ws, raw.intervalMs as number)
            return
          }

          // Channel response routing (requestId-correlated responses → channel, not FSM)
          if (raw.requestId) {
            const rtype = raw.type as string
            if (
              rtype === 'query_result' ||
              rtype === 'rewind_result' ||
              rtype === 'mcp_set_result'
            ) {
              channelRef.current.handleResponse(
                raw.requestId as string,
                raw.data ?? raw.result ?? raw,
              )
            }
          }

          // Map to FSM event
          const event = mapWsEvent(raw)
          if (event) dispatch(event)
        }

        ws.onclose = (ev) => {
          clearHeartbeat()
          channelRef.current.updateSend(null)
          channelRef.current.handleDisconnect()
          const recoverable = !(NON_RECOVERABLE_CODES as ReadonlySet<number>).has(ev.code)
          dispatch({ type: 'WS_CLOSE', code: ev.code, recoverable })
        }
        break
      }
      case 'CLOSE_SIDECAR_WS': {
        if (wsRef.current) {
          wsRef.current.onclose = null
          wsRef.current.close()
          wsRef.current = null
        }
        clearHeartbeat()
        channelRef.current.updateSend(null)
        break
      }
      case 'OPEN_TERMINAL_WS': {
        if (terminalWsRef.current) {
          terminalWsRef.current.onclose = null
          terminalWsRef.current.close()
        }
        // Terminal WS uses wsUrl (Rust server), not sidecarWsUrl.
        // Connect in block mode to stream ConversationBlocks for watching mode.
        const tws = new WebSocket(wsUrl(`/api/live/sessions/${cmd.sessionId}/terminal`))
        terminalWsRef.current = tws

        tws.onopen = () => {
          // Handshake: block mode, scrollback=0 (FETCH_HISTORY already loaded history)
          tws.send(JSON.stringify({ mode: 'block', scrollback: 0 }))
        }

        tws.onmessage = (ev) => {
          try {
            const parsed = JSON.parse(ev.data)
            if (parsed.type === 'buffer_end') {
              dispatch({ type: 'TERMINAL_CONNECTED' })
              return
            }
            if (parsed.type === 'pong' || parsed.type === 'error') return
            // Block mode: each message is a ConversationBlock (no wrapper type field)
            if (parsed.id && parsed.type) {
              dispatch({ type: 'TERMINAL_BLOCK', block: parsed })
            }
          } catch {
            // Not JSON — ignore
          }
        }
        break
      }
      case 'CLOSE_TERMINAL_WS': {
        if (terminalWsRef.current) {
          terminalWsRef.current.onclose = null
          terminalWsRef.current.close()
          terminalWsRef.current = null
        }
        break
      }
      case 'WS_SEND': {
        if (wsRef.current?.readyState === WebSocket.OPEN) {
          wsRef.current.send(JSON.stringify(cmd.message))
        }
        break
      }
      case 'INVALIDATE_SIDEBAR':
        queryClient.invalidateQueries({ queryKey: ['chat-sidebar-sessions'] })
        break
      case 'START_TIMER': {
        const existing = timersRef.current.get(cmd.id)
        if (existing) clearTimeout(existing)
        const timer = setTimeout(() => {
          timersRef.current.delete(cmd.id)
          dispatch(cmd.event)
        }, cmd.delayMs)
        timersRef.current.set(cmd.id, timer)
        break
      }
      case 'CANCEL_TIMER': {
        const timer = timersRef.current.get(cmd.id)
        if (timer) {
          clearTimeout(timer)
          timersRef.current.delete(cmd.id)
        }
        break
      }
      case 'TOAST':
        toast[cmd.variant](cmd.message)
        break
      case 'NAVIGATE':
        navigate(cmd.path)
        break
      case 'TRACK_EVENT':
        // analytics tracking placeholder
        break
    }
  }

  // Heartbeat
  function startHeartbeat(ws: WebSocket, intervalMs = HEARTBEAT_INTERVAL_MS) {
    clearHeartbeat()
    pongReceivedRef.current = true
    heartbeatRef.current = setInterval(() => {
      if (!pongReceivedRef.current) {
        ws.close(4200, 'heartbeat_timeout')
        return
      }
      pongReceivedRef.current = false
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'ping' }))
      }
    }, intervalMs)
  }

  function clearHeartbeat() {
    if (heartbeatRef.current) {
      clearInterval(heartbeatRef.current)
      heartbeatRef.current = null
    }
  }

  // E-B4: Sync channel send function with WS state
  useEffect(() => {
    const ws = wsRef.current
    channelRef.current.updateSend(
      ws?.readyState === WebSocket.OPEN ? (msg) => ws.send(JSON.stringify(msg)) : null,
    )
  }, [store.panel.phase])

  // E-m4: beforeunload — terminate SDK session on page close/refresh
  useEffect(() => {
    const handleUnload = () => {
      if (store.panel.phase === 'sdk_owned') {
        const p = store.panel as Extract<typeof store.panel, { phase: 'sdk_owned' }>
        fetch(`/api/sidecar/sessions/${p.controlId}`, {
          method: 'DELETE',
          keepalive: true,
        }).catch(() => {})
      }
    }
    window.addEventListener('beforeunload', handleUnload)
    return () => window.removeEventListener('beforeunload', handleUnload)
  }, [store.panel])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (wsRef.current) wsRef.current.close()
      if (terminalWsRef.current) terminalWsRef.current.close()
      for (const timer of timersRef.current.values()) clearTimeout(timer)
      clearHeartbeat()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentional mount-only cleanup
  }, [])

  return { channel: channelRef.current }
}
