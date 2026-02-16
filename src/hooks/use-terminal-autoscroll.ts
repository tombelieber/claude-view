import { useCallback } from 'react'
import type { Terminal } from '@xterm/xterm'

/**
 * useTerminalAutoScroll — Scrolls an xterm.js terminal to bottom after new content.
 *
 * Uses the official `terminal.scrollToBottom()` API, which properly fires scroll
 * events and triggers viewport repaint. The previous implementation directly mutated
 * `buffer.active.ydisp` with a wrong formula (`ybase + length - rows` overshoots),
 * which corrupted xterm.js's internal auto-scroll state and caused the viewport to
 * point at empty space past the end of content.
 *
 * Note: xterm.js already auto-scrolls when the viewport is at the bottom and new
 * content is written. This hook is only needed to force-scroll after the initial
 * scrollback buffer loads (when the viewport might not yet be at the bottom).
 *
 * Usage:
 *   const autoScroll = useTerminalAutoScroll()
 *   // After writing to terminal:
 *   terminal.write(data)
 *   autoScroll(terminal)
 */
export function useTerminalAutoScroll() {
  return useCallback((terminal: Terminal | null) => {
    if (!terminal) return
    terminal.scrollToBottom()
  }, [])
}
