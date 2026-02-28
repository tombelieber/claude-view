import {
  type CodeRenderContextValue,
  CodeRenderProvider,
} from '@claude-view/shared/contexts/CodeRenderContext'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BashProgressCard } from './BashProgressCard'

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

describe('BashProgressCard', () => {
  describe('Status rendering', () => {
    it('should display exit code and duration', () => {
      renderWithCodeContext(
        <BashProgressCard
          command="npm test"
          output="All tests passed"
          exitCode={0}
          duration={342}
        />,
      )

      expect(screen.getByText(/exit 0/)).toBeInTheDocument()
      expect(screen.getByText(/342ms/)).toBeInTheDocument()
    })

    it('should show green styling for exit code 0', () => {
      const { container } = renderWithCodeContext(<BashProgressCard command="ls" exitCode={0} />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-green')
    })

    it('should show red styling for non-zero exit code', () => {
      const { container } = renderWithCodeContext(
        <BashProgressCard command="bad-cmd" exitCode={1} />,
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-red')
    })
  })

  describe('Command rendering', () => {
    it('should render command via CompactCodeBlock with bash language', () => {
      renderWithCodeContext(<BashProgressCard command="npm test" exitCode={0} />)

      const codeBlocks = screen.getAllByTestId('compact-code-block')
      const cmdBlock = codeBlocks[0]
      expect(cmdBlock).toHaveAttribute('data-language', 'bash')
      expect(cmdBlock).toHaveTextContent('npm test')
    })
  })

  describe('Output rendering', () => {
    it('should show output immediately without expand click', () => {
      renderWithCodeContext(
        <BashProgressCard
          command="npm test"
          output="All tests passed\n5 tests, 0 failures"
          exitCode={0}
        />,
      )

      expect(screen.getByText(/All tests passed/)).toBeInTheDocument()
    })

    it('should render output via CompactCodeBlock with bash language', () => {
      renderWithCodeContext(<BashProgressCard command="echo hello" output="hello" exitCode={0} />)

      const codeBlocks = screen.getAllByTestId('compact-code-block')
      // First is command, second is output
      expect(codeBlocks).toHaveLength(2)
      expect(codeBlocks[1]).toHaveAttribute('data-language', 'bash')
      expect(codeBlocks[1]).toHaveTextContent('hello')
    })

    it('should show "No output" when output is empty string', () => {
      renderWithCodeContext(<BashProgressCard command="touch file.txt" output="" exitCode={0} />)

      expect(screen.getByText('No output')).toBeInTheDocument()
    })

    it('should only render command code block when output is undefined', () => {
      renderWithCodeContext(<BashProgressCard command="running..." />)

      const codeBlocks = screen.getAllByTestId('compact-code-block')
      expect(codeBlocks).toHaveLength(1)
      expect(codeBlocks[0]).toHaveTextContent('running...')
    })
  })

  describe('Edge cases', () => {
    it('should not show exit status when exitCode is undefined', () => {
      renderWithCodeContext(<BashProgressCard command="running..." />)

      expect(screen.queryByText(/exit/)).not.toBeInTheDocument()
    })

    it('should not show duration when undefined', () => {
      renderWithCodeContext(<BashProgressCard command="ls" exitCode={0} />)

      expect(screen.queryByText(/ms/)).not.toBeInTheDocument()
    })

    it('should render with only command prop', () => {
      const { container } = renderWithCodeContext(<BashProgressCard command="echo hello" />)
      expect(container).toBeInTheDocument()
      expect(screen.getByText('echo hello')).toBeInTheDocument()
    })
  })
})
