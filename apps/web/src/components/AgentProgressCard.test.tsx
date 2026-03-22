import {
  type CodeRenderContextValue,
  CodeRenderProvider,
} from '@claude-view/shared/contexts/CodeRenderContext'
import { render, screen } from '@testing-library/react'
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
  describe('All fields visible by default', () => {
    it('should display agent ID in header', () => {
      renderWithCodeContext(<AgentProgressCard agentId="agent-1" prompt="Research auth" />)

      expect(screen.getByText('#agent-1')).toBeInTheDocument()
    })

    it('should display prompt immediately without click', () => {
      renderWithCodeContext(
        <AgentProgressCard agentId="a1" prompt="This is the full prompt text" />,
      )

      const promptBlock = screen.getByTestId('agent-prompt')
      expect(promptBlock).toBeInTheDocument()
      expect(promptBlock.textContent).toContain('This is the full prompt text')
    })

    it('should show message as JSON when present', () => {
      renderWithCodeContext(
        <AgentProgressCard
          agentId="a1"
          prompt="Do something"
          message={{ status: 'completed', count: 42 }}
        />,
      )

      const msgBlock = screen.getByTestId('agent-message')
      expect(msgBlock.textContent).toContain('"status"')
    })

    it('should show string message as-is', () => {
      renderWithCodeContext(<AgentProgressCard agentId="a1" prompt="Do" message="plain text" />)

      expect(screen.getByText('plain text')).toBeInTheDocument()
    })

    it('should not show message section when undefined', () => {
      renderWithCodeContext(<AgentProgressCard agentId="a1" prompt="Do" />)

      expect(screen.queryByTestId('agent-message')).not.toBeInTheDocument()
    })

    it('should truncate prompt longer than 2000 chars', () => {
      const longPrompt = 'A'.repeat(2500)
      renderWithCodeContext(<AgentProgressCard agentId="a1" prompt={longPrompt} />)

      const promptBlock = screen.getByTestId('agent-prompt')
      expect(promptBlock.textContent!.length).toBeLessThan(2500)
    })
  })

  describe('Visual styling', () => {
    it('should have indigo left border', () => {
      const { container } = renderWithCodeContext(<AgentProgressCard agentId="a1" prompt="Do" />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('space-y')
    })

    it('should have aria-hidden on decorative icon', () => {
      const { container } = renderWithCodeContext(<AgentProgressCard agentId="a1" prompt="Do" />)

      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
