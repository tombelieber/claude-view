import {
  type CodeRenderContextValue,
  CodeRenderProvider,
} from '@claude-view/shared/contexts/CodeRenderContext'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { HookProgressCard } from './HookProgressCard'

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

describe('HookProgressCard', () => {
  describe('Title and status rendering', () => {
    it('should display hook event and command name', () => {
      renderWithCodeContext(
        <HookProgressCard hookEvent="SessionStart" hookName="pre-session" command="setup-env.sh" />,
      )

      expect(screen.getByText(/SessionStart/)).toBeInTheDocument()
      expect(screen.getByText(/setup-env\.sh/)).toBeInTheDocument()
    })

    it('should have amber left border', () => {
      const { container } = renderWithCodeContext(
        <HookProgressCard hookEvent="SessionStart" hookName="pre-session" command="cmd" />,
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-amber')
    })
  })

  describe('Output rendering', () => {
    it('should show output immediately via CompactCodeBlock', () => {
      renderWithCodeContext(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup.sh"
          output="Environment configured successfully"
        />,
      )

      expect(screen.getByText(/Environment configured successfully/)).toBeInTheDocument()
    })

    it('should render output via CompactCodeBlock with bash language', () => {
      renderWithCodeContext(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup.sh"
          output="some output"
        />,
      )

      const codeBlock = screen.getByTestId('compact-code-block')
      expect(codeBlock).toHaveAttribute('data-language', 'bash')
      expect(codeBlock).toHaveTextContent('some output')
    })

    it('should not render code block when output is undefined', () => {
      renderWithCodeContext(
        <HookProgressCard hookEvent="SessionStart" hookName="pre-session" command="setup.sh" />,
      )

      expect(screen.queryByTestId('compact-code-block')).not.toBeInTheDocument()
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = renderWithCodeContext(
        <HookProgressCard hookEvent="SessionStart" hookName="pre-session" command="cmd" />,
      )
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
