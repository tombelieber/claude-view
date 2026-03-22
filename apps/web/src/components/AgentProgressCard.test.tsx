import {
  type CodeRenderContextValue,
  CodeRenderProvider,
} from '@claude-view/shared/contexts/CodeRenderContext'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { AgentProgressCard } from './AgentProgressCard'

const MockCompactCodeBlock = ({
  code,
  language,
  blockId,
}: { code: string; language: string; blockId?: string }) => (
  <pre data-testid="compact-code-block" data-language={language} data-block-id={blockId}>
    {code}
  </pre>
)

const MockCodeBlock = ({ code, language }: { code: string; language?: string | null }) => (
  <pre data-testid="code-block" data-language={language}>
    {code}
  </pre>
)

const mockCodeRender: CodeRenderContextValue = {
  CodeBlock: MockCodeBlock as any,
  CompactCodeBlock: MockCompactCodeBlock as any,
}

function renderWithCodeContext(ui: React.ReactElement) {
  return render(<CodeRenderProvider value={mockCodeRender}>{ui}</CodeRenderProvider>)
}

describe('AgentProgressCard', () => {
  describe('Header rendering', () => {
    it('should display agent ID and prompt preview', () => {
      renderWithCodeContext(
        <AgentProgressCard agentId="agent-1" prompt="Research auth best practices" />,
      )

      expect(screen.getByText('#agent-1')).toBeInTheDocument()
      expect(screen.getByText(/Research auth best practices/)).toBeInTheDocument()
    })

    it('should truncate long prompts in header to 80 chars', () => {
      const longPrompt = 'A'.repeat(200)
      renderWithCodeContext(<AgentProgressCard agentId="a1" prompt={longPrompt} />)

      // Header should show truncated version
      const headerText = screen.getByText('A'.repeat(80))
      expect(headerText).toBeInTheDocument()
    })

    it('should show +msg badge when message is present', () => {
      renderWithCodeContext(
        <AgentProgressCard agentId="a1" prompt="Do something" message={{ result: 'done' }} />,
      )

      expect(screen.getByText('+msg')).toBeInTheDocument()
    })

    it('should not show +msg badge when message is undefined', () => {
      renderWithCodeContext(<AgentProgressCard agentId="a1" prompt="Do something" />)

      expect(screen.queryByText('+msg')).not.toBeInTheDocument()
    })
  })

  describe('Collapsible behavior', () => {
    it('should expand to show full prompt on click', () => {
      renderWithCodeContext(
        <AgentProgressCard agentId="a1" prompt="This is the full prompt text" />,
      )

      // Prompt details not visible initially
      expect(screen.queryByTestId('agent-prompt')).not.toBeInTheDocument()

      fireEvent.click(screen.getByRole('button'))

      const promptBlock = screen.getByTestId('agent-prompt')
      expect(promptBlock).toBeInTheDocument()
      expect(promptBlock.textContent).toContain('This is the full prompt text')
    })

    it('should truncate prompt longer than 1000 chars in expanded view', () => {
      const longPrompt = 'A'.repeat(1200)
      renderWithCodeContext(<AgentProgressCard agentId="a1" prompt={longPrompt} />)

      fireEvent.click(screen.getByRole('button'))

      const truncated = screen.getByTestId('agent-prompt')
      expect(truncated.textContent!.length).toBeLessThan(1200)
    })

    it('should show message as JSON in expanded view', () => {
      renderWithCodeContext(
        <AgentProgressCard
          agentId="a1"
          prompt="Do something"
          message={{ status: 'completed', count: 42 }}
        />,
      )

      fireEvent.click(screen.getByRole('button'))

      const msgBlock = screen.getByTestId('agent-message')
      expect(msgBlock).toBeInTheDocument()
      expect(msgBlock.textContent).toContain('"status"')
      expect(msgBlock.textContent).toContain('"completed"')
    })

    it('should show string message as-is', () => {
      renderWithCodeContext(
        <AgentProgressCard agentId="a1" prompt="Do something" message="plain text result" />,
      )

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText('plain text result')).toBeInTheDocument()
    })
  })

  describe('XSS prevention (React auto-escaping)', () => {
    it('should properly escape text content with script tags', () => {
      renderWithCodeContext(
        <AgentProgressCard
          agentId='<script>alert("XSS")</script>'
          prompt='Test prompt with <img src=x onerror="alert(1)">'
        />,
      )

      expect(screen.getByText(/<script>alert\("XSS"\)<\/script>/)).toBeInTheDocument()
    })
  })

  describe('Visual styling', () => {
    it('should have indigo left border', () => {
      const { container } = renderWithCodeContext(<AgentProgressCard agentId="a1" prompt="Do" />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-indigo')
    })
  })

  describe('ARIA and keyboard', () => {
    it('should have ARIA label on the expand button', () => {
      renderWithCodeContext(<AgentProgressCard agentId="a1" prompt="Do" />)

      expect(screen.getByRole('button', { name: /agent progress/i })).toBeInTheDocument()
    })
  })
})
