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

  describe('Header rendering', () => {
    it('should display hookEvent and hookName', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} />)

      expect(screen.getByText('PreToolUse')).toBeInTheDocument()
      expect(screen.getByText('live-monitor')).toBeInTheDocument()
    })

    it('should show arrow between event and name', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} />)

      expect(screen.getByText('\u2192')).toBeInTheDocument()
    })
  })

  describe('Command rendering', () => {
    it('should render command via CompactCodeBlock with bash language', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} />)

      const codeBlock = screen.getByTestId('compact-code-block')
      expect(codeBlock).toHaveAttribute('data-language', 'bash')
      expect(codeBlock).toHaveTextContent('/Users/dev/.claude/hooks/pre-tool.sh')
    })
  })

  describe('Status message rendering', () => {
    it('should display statusMessage below the command', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} />)

      expect(screen.getByText('Validating tool use\u2026')).toBeInTheDocument()
    })

    it('should not render status area when statusMessage is empty', () => {
      renderWithCodeContext(<HookProgressCard {...baseProps} statusMessage="" />)

      // Only the code block and header should render, no extra text node
      const allText = screen.queryByText('Validating')
      expect(allText).not.toBeInTheDocument()
    })
  })

  describe('Visual styling', () => {
    it('should have amber left border', () => {
      const { container } = renderWithCodeContext(<HookProgressCard {...baseProps} />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-amber')
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = renderWithCodeContext(<HookProgressCard {...baseProps} />)

      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
