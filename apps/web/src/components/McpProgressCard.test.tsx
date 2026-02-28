import {
  type CodeRenderContextValue,
  CodeRenderProvider,
} from '@claude-view/shared/contexts/CodeRenderContext'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { McpProgressCard } from './McpProgressCard'

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

describe('McpProgressCard', () => {
  describe('Title and status rendering', () => {
    it('should display server.method with params', () => {
      renderWithCodeContext(
        <McpProgressCard
          server="filesystem"
          method="readFile"
          params={{ path: '/tmp/test.txt' }}
        />,
      )

      expect(screen.getByText(/filesystem\.readFile/)).toBeInTheDocument()
    })

    it('should show "(no params)" when params is undefined', () => {
      renderWithCodeContext(<McpProgressCard server="memory" method="search" />)

      expect(screen.getByText(/\(no params\)/)).toBeInTheDocument()
    })

    it('should have purple left border', () => {
      const { container } = renderWithCodeContext(<McpProgressCard server="fs" method="read" />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-purple')
    })
  })

  describe('Collapsible behavior', () => {
    it('should expand to show result on click', () => {
      renderWithCodeContext(
        <McpProgressCard
          server="filesystem"
          method="readFile"
          params={{ path: '/tmp/test.txt' }}
          result={{ content: 'file contents here' }}
        />,
      )

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText(/file contents here/)).toBeInTheDocument()
    })

    it('should show params in expanded view', () => {
      renderWithCodeContext(
        <McpProgressCard server="fs" method="read" params={{ path: '/tmp/test.txt' }} />,
      )

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText(/\/tmp\/test\.txt/)).toBeInTheDocument()
    })
  })

  describe('ARIA and keyboard', () => {
    it('should have ARIA label', () => {
      renderWithCodeContext(<McpProgressCard server="fs" method="read" />)
      expect(screen.getByRole('button', { name: /mcp/i })).toBeInTheDocument()
    })
  })
})
