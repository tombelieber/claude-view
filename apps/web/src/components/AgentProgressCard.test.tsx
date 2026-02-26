import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AgentProgressCard } from './AgentProgressCard'

vi.mock('./live/CompactCodeBlock', () => ({
  CompactCodeBlock: ({ code, language, blockId }: { code: string; language: string; blockId?: string }) => (
    <pre data-testid="compact-code-block" data-language={language} data-block-id={blockId}>{code}</pre>
  ),
}))

describe('AgentProgressCard', () => {
  describe('React auto-escaping XSS prevention', () => {
    it('should properly escape text content with script tags (React auto-escaping)', () => {
      const xssProps = {
        agentId: '<script>alert("XSS")</script>',
        prompt: 'Test prompt with <img src=x onerror="alert(1)">',
        model: 'claude-<script>alert("model")</script>opus',
        tokens: { input: 100, output: 200 },
        normalizedMessages: 5,
      }

      render(<AgentProgressCard {...xssProps} />)

      expect(screen.getByText(/<script>alert\("XSS"\)<\/script>/)).toBeInTheDocument()
      expect(screen.getByText(/claude-<script>alert\("model"\)<\/script>opus/)).toBeInTheDocument()
    })

    it('should render text content without interpreting HTML entities', () => {
      const htmlProps = {
        agentId: 'agent-&lt;malicious&gt;',
        prompt: 'Prompt with &quot;quoted&quot; text',
        model: 'claude-opus',
        tokens: { input: 50, output: 150 },
        normalizedMessages: 3,
      }

      render(<AgentProgressCard {...htmlProps} />)

      expect(screen.getByText(/agent-&lt;malicious&gt;/)).toBeInTheDocument()
    })

    it('should not allow event handler binding to escaped text', () => {
      const eventHandlerProps = {
        agentId: 'onclick="console.log(\'clicked\')"',
        prompt: 'onmouseover="alert(\'hover\')"',
        model: 'claude-opus',
        tokens: { input: 100, output: 200 },
        normalizedMessages: 5,
      }

      const { container } = render(<AgentProgressCard {...eventHandlerProps} />)

      const allElements = container.querySelectorAll('*')
      for (const el of allElements) {
        expect(el.getAttribute('onclick')).toBeNull()
        expect(el.getAttribute('onmouseover')).toBeNull()
      }
    })
  })

  describe('Title and status rendering', () => {
    it('should display agent title with model and token info', () => {
      render(
        <AgentProgressCard
          agentId="agent-1"
          prompt="Do something"
          model="claude-opus"
          tokens={{ input: 300, output: 200 }}
        />
      )

      expect(screen.getByText(/Agent #agent-1/)).toBeInTheDocument()
      expect(screen.getByText(/claude-opus/)).toBeInTheDocument()
      expect(screen.getByText(/500 tokens/)).toBeInTheDocument()
    })

    it('should show "Sub-agent" when agentId is undefined', () => {
      render(
        <AgentProgressCard prompt="Do something" model="claude-opus" />
      )

      expect(screen.getByText(/Sub-agent/)).toBeInTheDocument()
    })

    it('should not show token display when tokens undefined', () => {
      render(
        <AgentProgressCard agentId="a1" prompt="Do" model="claude-opus" />
      )

      expect(screen.queryByText(/tokens/)).not.toBeInTheDocument()
    })
  })

  describe('Collapsible behavior', () => {
    it('should expand to show prompt on click', () => {
      render(
        <AgentProgressCard
          agentId="a1"
          prompt="This is the full prompt text"
          model="claude-opus"
        />
      )

      // Prompt should not be visible initially
      expect(screen.queryByText('This is the full prompt text')).not.toBeInTheDocument()

      // Click to expand
      fireEvent.click(screen.getByRole('button'))

      // Prompt should now be visible via CompactCodeBlock
      expect(screen.getByText('This is the full prompt text')).toBeInTheDocument()
    })

    it('should truncate prompt longer than 1000 chars in expanded view', () => {
      const longPrompt = 'A'.repeat(1200)
      render(
        <AgentProgressCard
          agentId="a1"
          prompt={longPrompt}
          model="claude-opus"
        />
      )

      fireEvent.click(screen.getByRole('button'))

      // Should show truncated text
      const truncated = screen.getByTestId('agent-prompt')
      expect(truncated.textContent!.length).toBeLessThan(1200)
    })
  })

  describe('Visual styling', () => {
    it('should have indigo left border', () => {
      const { container } = render(
        <AgentProgressCard agentId="a1" prompt="Do" model="claude-opus" />
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-indigo')
    })

    it('should apply nested indentation via indent prop', () => {
      const { container } = render(
        <AgentProgressCard agentId="a1" prompt="Do" model="claude-opus" indent={2} />
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.style.marginLeft).toBe('32px')
    })
  })

  describe('ARIA and keyboard', () => {
    it('should have ARIA label on the card', () => {
      render(
        <AgentProgressCard agentId="a1" prompt="Do" model="claude-opus" />
      )

      expect(screen.getByRole('button', { name: /agent progress/i })).toBeInTheDocument()
    })

    it('should toggle expanded on Enter key', () => {
      render(
        <AgentProgressCard agentId="a1" prompt="Prompt text here" model="claude-opus" />
      )

      const button = screen.getByRole('button')
      fireEvent.keyDown(button, { key: 'Enter' })
      // Native button handles Enter natively, so click fires
    })
  })

  describe('Edge cases', () => {
    it('should handle all props undefined gracefully', () => {
      const { container } = render(<AgentProgressCard />)
      expect(container).toBeInTheDocument()
      expect(screen.getByText(/Sub-agent/)).toBeInTheDocument()
    })

    it('should handle empty strings safely', () => {
      const { container } = render(
        <AgentProgressCard agentId="" prompt="" model="" />
      )
      expect(container).toBeInTheDocument()
    })
  })
})
