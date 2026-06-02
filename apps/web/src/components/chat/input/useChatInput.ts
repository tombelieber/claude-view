import { useCallback, useRef, useState } from 'react'
import type { PermissionMode } from '../../../types/control'
import type { SlashCommand } from '../commands'
import { cycleMode } from '../ModeSwitch'
import { MODE_COMMANDS } from './input-bar-state'

interface UseChatInputArgs {
  onSend: (message: string) => void
  onStop?: () => void
  isDisabled: boolean
  isStreaming: boolean
  mode: PermissionMode
  onModeChange?: (mode: PermissionMode) => void
  onCommand?: (command: string) => void
  onPaletteOpen?: () => void
}

/**
 * Owns the input bar's local state (draft text, slash-popover open, image
 * attachments, textarea ref) and the keyboard/paste/slash handlers. Extracted
 * from ChatInputBar so the component is a thin view over this logic — behavior
 * is identical, just relocated.
 */
export function useChatInput({
  onSend,
  onStop,
  isDisabled,
  isStreaming,
  mode,
  onModeChange,
  onCommand,
  onPaletteOpen,
}: UseChatInputArgs) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const [input, setInput] = useState('')
  const [slashOpen, setSlashOpen] = useState(false)
  const [attachments, setAttachments] = useState<File[]>([])

  // ---- Auto-grow textarea ----
  const autoGrow = useCallback(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`
  }, [])

  // ---- Send logic ----
  const send = useCallback(() => {
    const trimmed = input.trim()
    if (!trimmed || isDisabled) return
    onSend(trimmed)
    setInput('')
    setAttachments([])
    requestAnimationFrame(() => {
      const el = textareaRef.current
      if (el) {
        el.style.height = 'auto'
      }
    })
  }, [input, isDisabled, onSend])

  // ---- Slash command handling ----
  const handleSlashSelect = useCallback(
    (cmd: SlashCommand) => {
      if (MODE_COMMANDS.has(cmd.name) && onModeChange) {
        onModeChange(cmd.name as PermissionMode)
      } else if (onCommand) {
        onCommand(cmd.name)
      }
      setInput('')
      setSlashOpen(false)
      requestAnimationFrame(() => {
        const el = textareaRef.current
        if (el) {
          el.style.height = 'auto'
          el.focus()
        }
      })
    },
    [onModeChange, onCommand],
  )

  // ---- Input change ----
  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const value = e.target.value
      setInput(value)

      // Open slash popover when input starts with "/" (single-line, at start)
      const trimmed = value.trimStart()
      if (trimmed.startsWith('/') && !trimmed.includes('\n')) {
        if (!slashOpen) onPaletteOpen?.()
        setSlashOpen(true)
      } else {
        setSlashOpen(false)
      }

      requestAnimationFrame(autoGrow)
    },
    [autoGrow, slashOpen, onPaletteOpen],
  )

  // ---- Keyboard: Enter=send, Shift+Enter=newline, Escape=stop ----
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Don't intercept keys when slash popover is open
      // (the popover handles its own keyboard events)
      if (slashOpen) return

      if (e.key === 'Tab' && e.shiftKey && onModeChange) {
        e.preventDefault()
        onModeChange(cycleMode(mode))
      } else if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        send()
      } else if (e.key === 'Escape' && isStreaming && onStop) {
        e.preventDefault()
        onStop()
      }
    },
    [slashOpen, send, isStreaming, onStop, mode, onModeChange],
  )

  // ---- Image paste handler ----
  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      if (isDisabled) return
      const items = e.clipboardData?.items
      if (!items) return

      const imageFiles: File[] = []
      for (let i = 0; i < items.length; i++) {
        const item = items[i]
        if (item.type.startsWith('image/')) {
          const file = item.getAsFile()
          if (file) imageFiles.push(file)
        }
      }

      if (imageFiles.length > 0) {
        setAttachments((prev) => [...prev, ...imageFiles])
      }
    },
    [isDisabled],
  )

  // ---- Attachments ----
  const handleAttach = useCallback((files: File[]) => {
    setAttachments((prev) => [...prev, ...files])
  }, [])

  const handleRemoveAttachment = useCallback((index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index))
  }, [])

  // ---- Palette close ----
  const handlePaletteClose = useCallback(() => {
    setSlashOpen(false)
    setInput('')
    requestAnimationFrame(() => {
      const el = textareaRef.current
      if (el) {
        el.style.height = 'auto'
        el.focus()
      }
    })
  }, [])

  const canSend = input.trim().length > 0 && !isDisabled

  return {
    textareaRef,
    input,
    slashOpen,
    setSlashOpen,
    attachments,
    send,
    canSend,
    handleSlashSelect,
    handleInputChange,
    handleKeyDown,
    handlePaste,
    handleAttach,
    handleRemoveAttachment,
    handlePaletteClose,
  }
}
