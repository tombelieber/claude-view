import { useCallback, useRef, useState } from 'react'

import type { WorkflowMode } from '../../pages/WorkflowDetailPage'

const MODE_LABELS: Record<WorkflowMode, string> = {
  design: 'Designing',
  control: 'Running',
  review: 'Review',
}

const MODE_PLACEHOLDERS: Record<WorkflowMode, string> = {
  design: 'Describe your workflow...',
  control: 'Pause, skip stage, abort...',
  review: 'Re-run, patch, or ask what failed',
}

interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
}

interface WorkflowChatRailProps {
  workflowId: string | null
  mode: WorkflowMode
  onModeChange: (mode: WorkflowMode) => void
  onYamlUpdate: (yaml: string) => void
  onWorkflowGenerated: () => void
  runId: string | null
  autoMessage: string | null
  generatedYaml: string
}

function extractYaml(content: string): string | undefined {
  const match = content.match(/```ya?ml\n([\s\S]*?)```/)
  return match?.[1]?.trim()
}

export function WorkflowChatRail({ mode, workflowId, onYamlUpdate }: WorkflowChatRailProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [inputValue, setInputValue] = useState('')
  const [isStreaming, setIsStreaming] = useState(false)

  const abortRef = useRef<AbortController | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const scrollToBottom = useCallback(() => {
    const el = scrollRef.current
    if (el) {
      el.scrollTop = el.scrollHeight
    }
  }, [])

  const handleDesignSubmit = useCallback(async () => {
    const text = inputValue.trim()
    if (!text || isStreaming) return

    const userMessage: ChatMessage = {
      id: `user-${Date.now()}`,
      role: 'user',
      content: text,
    }

    const allMessages = [...messages, userMessage]
    setMessages(allMessages)
    setInputValue('')
    setIsStreaming(true)
    scrollToBottom()

    const assistantId = `assistant-${Date.now()}`
    const assistantMessage: ChatMessage = {
      id: assistantId,
      role: 'assistant',
      content: '',
    }
    setMessages((prev) => [...prev, assistantMessage])
    scrollToBottom()

    const controller = new AbortController()
    abortRef.current = controller

    try {
      const res = await fetch('/api/workflows/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          messages: allMessages.map((m) => ({
            role: m.role,
            content: m.content,
          })),
          workflowId,
        }),
        signal: controller.signal,
      })

      if (!res.ok) {
        const errText = await res.text()
        throw new Error(errText || `HTTP ${res.status}`)
      }

      const reader = res.body?.getReader()
      if (!reader) throw new Error('No response body')

      const decoder = new TextDecoder()
      let buffer = ''
      let accumulated = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })

        // Parse SSE events from buffer
        const lines = buffer.split('\n')
        buffer = lines.pop() || '' // Keep incomplete line in buffer

        let eventType = ''
        for (const line of lines) {
          if (line.startsWith('event: ')) {
            eventType = line.slice(7).trim()
          } else if (line.startsWith('data: ')) {
            const data = line.slice(6)
            try {
              const parsed = JSON.parse(data)

              if (eventType === 'chunk') {
                accumulated += parsed.text
                const updated = accumulated
                setMessages((prev) =>
                  prev.map((m) => (m.id === assistantId ? { ...m, content: updated } : m)),
                )
                scrollToBottom()

                // Extract YAML and notify parent
                const yaml = extractYaml(accumulated)
                if (yaml) {
                  onYamlUpdate(yaml)
                }
              } else if (eventType === 'done') {
                setIsStreaming(false)
              } else if (eventType === 'error') {
                const errContent = parsed.message || 'Generation failed'
                setMessages((prev) =>
                  prev.map((m) =>
                    m.id === assistantId ? { ...m, content: `Error: ${errContent}` } : m,
                  ),
                )
                setIsStreaming(false)
              }
            } catch {
              // Skip unparseable lines
            }
          }
        }
      }

      setIsStreaming(false)
    } catch (err: unknown) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        setIsStreaming(false)
        return
      }
      const errMsg = err instanceof Error ? err.message : 'Unknown error'
      setMessages((prev) =>
        prev.map((m) => (m.id === assistantId ? { ...m, content: `Error: ${errMsg}` } : m)),
      )
      setIsStreaming(false)
    }
  }, [inputValue, isStreaming, messages, workflowId, onYamlUpdate, scrollToBottom])

  const handleCancel = useCallback(() => {
    abortRef.current?.abort()
    abortRef.current = null
  }, [])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleDesignSubmit()
      }
    },
    [handleDesignSubmit],
  )

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-800 flex items-center justify-between">
        <span className="text-xs text-gray-500 dark:text-gray-400">{MODE_LABELS[mode]}</span>
        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
          {workflowId ?? 'New Workflow'}
        </span>
      </div>

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-3 space-y-3">
        {messages.length === 0 && (
          <p className="text-center py-8 text-xs text-gray-400 dark:text-gray-500">
            {mode === 'design' ? 'Describe the workflow you want to create.' : ''}
          </p>
        )}

        {messages.map((msg) => (
          <div
            key={msg.id}
            className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
          >
            <div
              className={`max-w-[85%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap ${
                msg.role === 'user'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
              }`}
            >
              {msg.content || (
                <span className="text-gray-400 dark:text-gray-500 animate-pulse">...</span>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Input area */}
      <div className="p-3 border-t border-gray-200 dark:border-gray-800">
        <div className="relative">
          <textarea
            ref={textareaRef}
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={MODE_PLACEHOLDERS[mode]}
            disabled={isStreaming || mode !== 'design'}
            rows={3}
            className="w-full resize-none rounded-md border border-gray-200 dark:border-gray-700
                       bg-transparent text-sm px-3 py-2 pr-16
                       placeholder:text-gray-400 dark:placeholder:text-gray-600
                       focus:outline-none focus:ring-1 focus:ring-gray-400
                       disabled:opacity-50"
          />
          <div className="absolute right-2 bottom-2 flex gap-1">
            {isStreaming ? (
              <button
                type="button"
                onClick={handleCancel}
                className="px-2 py-1 text-xs rounded bg-red-500 text-white
                           hover:bg-red-600 transition-colors"
              >
                Stop
              </button>
            ) : (
              <button
                type="button"
                onClick={handleDesignSubmit}
                disabled={!inputValue.trim() || mode !== 'design'}
                className="px-2 py-1 text-xs rounded bg-blue-600 text-white
                           hover:bg-blue-700 transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Send
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
