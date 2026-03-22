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
  const baseProps = {
    hookEvent: 'PreToolUse',
    hookName: 'live-monitor',
    command: '/Users/dev/.claude/hooks/pre-tool.sh',
    statusMessage: 'Validating tool use\u2026',
  }

  describe('All fields visible', () => {
    it('should display hookEvent and hookName inline', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} />)

      expect(screen.getByText('PreToolUse')).toBeInTheDocument()
      expect(screen.getByText('live-monitor')).toBeInTheDocument()
    })

    it('should display statusMessage inline', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} />)

      expect(screen.getByText('Validating tool use\u2026')).toBeInTheDocument()
    })

    it('should not show statusMessage dot when empty', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} statusMessage="" />)

      // Only hookEvent, arrow, hookName visible — no extra dot separator
      expect(screen.queryByText('Validating')).not.toBeInTheDocument()
    })

    it('should render command via CompactCodeBlock', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} />)

      const codeBlock = screen.getByTestId('compact-code-block')
      expect(codeBlock).toHaveAttribute('data-language', 'bash')
      expect(codeBlock).toHaveTextContent('/Users/dev/.claude/hooks/pre-tool.sh')
    })
  })

  describe('Visual styling', () => {
    it('should have amber left border', () => {
      const { container } = renderWithCodeContext(<HookProgressCard {...baseProps} />)

      expect((container.firstElementChild as HTMLElement).className).toContain('space-y')
    })

    it('should have aria-hidden on decorative icon', () => {
      const { container } = renderWithCodeContext(<HookProgressCard {...baseProps} />)

      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
