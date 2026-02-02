import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MessageTyped } from './MessageTyped'
import type { Message as MessageType } from '../hooks/use-session'

// Mock all card components so we can verify dispatch without needing their internals
vi.mock('./TurnDurationCard', () => ({
  TurnDurationCard: (props: any) => <div data-testid="turn-duration-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./ApiErrorCard', () => ({
  ApiErrorCard: (props: any) => <div data-testid="api-error-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./CompactBoundaryCard', () => ({
  CompactBoundaryCard: (props: any) => <div data-testid="compact-boundary-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./HookSummaryCard', () => ({
  HookSummaryCard: (props: any) => <div data-testid="hook-summary-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./LocalCommandEventCard', () => ({
  LocalCommandEventCard: (props: any) => <div data-testid="local-command-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./AgentProgressCard', () => ({
  AgentProgressCard: (props: any) => <div data-testid="agent-progress-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./BashProgressCard', () => ({
  BashProgressCard: (props: any) => <div data-testid="bash-progress-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./HookProgressCard', () => ({
  HookProgressCard: (props: any) => <div data-testid="hook-progress-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./McpProgressCard', () => ({
  McpProgressCard: (props: any) => <div data-testid="mcp-progress-card" data-props={JSON.stringify(props)} />
}))
vi.mock('./TaskQueueCard', () => ({
  TaskQueueCard: (props: any) => <div data-testid="task-queue-card" data-props={JSON.stringify(props)} />
}))

function makeMessage(overrides: Partial<MessageType> = {}): MessageType {
  return {
    role: 'system',
    content: '',
    timestamp: '2026-01-30T12:00:00Z',
    ...overrides,
  } as MessageType
}

describe('MessageTyped dispatch', () => {
  describe('System event subtypes', () => {
    it('dispatches turn_duration to TurnDurationCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ type: 'turn_duration', durationMs: 500, startTime: '10:00', endTime: '10:01' }}
        />
      )
      expect(screen.getByTestId('turn-duration-card')).toBeInTheDocument()
    })

    it('dispatches api_error to ApiErrorCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ type: 'api_error', error: { code: 500 }, retryAttempt: 1, maxRetries: 3 }}
        />
      )
      expect(screen.getByTestId('api-error-card')).toBeInTheDocument()
    })

    it('dispatches compact_boundary to CompactBoundaryCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ type: 'compact_boundary', trigger: 'auto', preTokens: 1000 }}
        />
      )
      expect(screen.getByTestId('compact-boundary-card')).toBeInTheDocument()
    })

    it('dispatches hook_summary to HookSummaryCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ type: 'hook_summary', hookCount: 2, hookInfos: [] }}
        />
      )
      expect(screen.getByTestId('hook-summary-card')).toBeInTheDocument()
    })

    it('dispatches local_command to LocalCommandEventCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ type: 'local_command', content: 'ls -la' }}
        />
      )
      expect(screen.getByTestId('local-command-card')).toBeInTheDocument()
    })

    it('uses metadata.subtype as fallback for system events', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ subtype: 'turn_duration', durationMs: 100 }}
        />
      )
      expect(screen.getByTestId('turn-duration-card')).toBeInTheDocument()
    })

    it('falls back to SystemMetadataCard for unknown system subtype', () => {
      const { container } = render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ type: 'unknown_system_event', someData: 'test' }}
        />
      )
      // Should NOT render any specialized card
      expect(screen.queryByTestId('turn-duration-card')).not.toBeInTheDocument()
      expect(screen.queryByTestId('api-error-card')).not.toBeInTheDocument()
      // SystemMetadataCard renders metadata keys; check for "someData"
      expect(container.textContent).toContain('someData')
    })

    it('falls back to SystemMetadataCard when no subtype is present', () => {
      const { container } = render(
        <MessageTyped
          message={makeMessage()}
          messageType="system"
          metadata={{ customField: 'value123' }}
        />
      )
      expect(container.textContent).toContain('customField')
      expect(container.textContent).toContain('value123')
    })
  })

  describe('Progress event subtypes', () => {
    it('dispatches agent_progress to AgentProgressCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="progress"
          metadata={{ type: 'agent_progress', agentId: 'a1', prompt: 'test', model: 'opus' }}
        />
      )
      expect(screen.getByTestId('agent-progress-card')).toBeInTheDocument()
    })

    it('dispatches bash_progress to BashProgressCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="progress"
          metadata={{ type: 'bash_progress', command: 'echo hi' }}
        />
      )
      expect(screen.getByTestId('bash-progress-card')).toBeInTheDocument()
    })

    it('dispatches hook_progress to HookProgressCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="progress"
          metadata={{ type: 'hook_progress', hookEvent: 'pre-commit', hookName: 'lint', command: 'eslint .' }}
        />
      )
      expect(screen.getByTestId('hook-progress-card')).toBeInTheDocument()
    })

    it('dispatches mcp_progress to McpProgressCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="progress"
          metadata={{ type: 'mcp_progress', server: 'supabase', method: 'query' }}
        />
      )
      expect(screen.getByTestId('mcp-progress-card')).toBeInTheDocument()
    })

    it('dispatches waiting_for_task to TaskQueueCard', () => {
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="progress"
          metadata={{ type: 'waiting_for_task', position: 3, queueLength: 10 }}
        />
      )
      expect(screen.getByTestId('task-queue-card')).toBeInTheDocument()
    })

    it('falls back to SystemMetadataCard for unknown progress subtype', () => {
      const { container } = render(
        <MessageTyped
          message={makeMessage()}
          messageType="progress"
          metadata={{ type: 'unknown_progress', detail: 'xyz' }}
        />
      )
      expect(screen.queryByTestId('agent-progress-card')).not.toBeInTheDocument()
      expect(container.textContent).toContain('detail')
    })
  })

  describe('Nesting depth warning', () => {
    it('should console.warn when indent exceeds MAX_INDENT_LEVEL', () => {
      const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})

      render(
        <MessageTyped
          message={{ uuid: '1', role: 'user', content: 'deep', timestamp: null, thinking: null, tool_calls: [], parent_uuid: null, metadata: null }}
          indent={10}
        />
      )

      expect(warnSpy).toHaveBeenCalledWith(
        expect.stringContaining('Max nesting depth')
      )
      warnSpy.mockRestore()
    })

    it('should NOT warn when indent is within MAX_INDENT_LEVEL', () => {
      const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})

      render(
        <MessageTyped
          message={{ uuid: '2', role: 'user', content: 'shallow', timestamp: null, thinking: null, tool_calls: [], parent_uuid: null, metadata: null }}
          indent={3}
        />
      )

      expect(warnSpy).not.toHaveBeenCalled()
      warnSpy.mockRestore()
    })
  })

  describe('Non-system/progress types pass through', () => {
    it('renders assistant message content normally', () => {
      render(
        <MessageTyped
          message={makeMessage({ role: 'assistant', content: 'Hello world' })}
          messageType="assistant"
        />
      )
      expect(screen.getByText('Hello world')).toBeInTheDocument()
    })

    it('renders user message content normally', () => {
      render(
        <MessageTyped
          message={makeMessage({ role: 'user', content: 'My question' })}
          messageType="user"
        />
      )
      expect(screen.getByText('My question')).toBeInTheDocument()
    })
  })
})
