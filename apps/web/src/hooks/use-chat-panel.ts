import { useEffect, useReducer, useRef } from 'react'
import {
  type ChatPanelStore,
  type Command,
  type RawEvent,
  coordinate,
  deriveBlocks,
  deriveCanFork,
  deriveCanSend,
  deriveInputBar,
  deriveViewMode,
} from '../lib/chat-panel'

const initialStore: ChatPanelStore = {
  panel: { phase: 'empty' },
  outbox: { messages: [] },
  meta: null,
}

/** E-B1: pendingCmdsRef OVERWRITES (not pushes) — safe under StrictMode double-invoke */
export function useChatPanel(sessionId: string | undefined) {
  const pendingCmdsRef = useRef<Command[]>([])

  function reducer(state: ChatPanelStore, event: RawEvent): ChatPanelStore {
    const [newState, cmds] = coordinate(state, event)
    pendingCmdsRef.current = cmds // OVERWRITE, not push
    return newState
  }

  const [store, dispatch] = useReducer(reducer, initialStore)

  // Derived state
  const blocks = deriveBlocks(store)
  const canSend = deriveCanSend(store)
  const canFork = deriveCanFork(store)
  const inputBar = deriveInputBar(store)
  const viewMode = deriveViewMode(store)

  // E-m3: Guard SELECT_SESSION re-dispatch
  useEffect(() => {
    if (sessionId) {
      if (
        store.panel.phase === 'empty' ||
        ('sessionId' in store.panel && store.panel.sessionId !== sessionId)
      ) {
        dispatch({ type: 'SELECT_SESSION', sessionId })
      }
    } else {
      dispatch({ type: 'DESELECT' })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- store intentionally excluded
  }, [sessionId])

  return { store, dispatch, pendingCmdsRef, blocks, canSend, canFork, inputBar, viewMode }
}
