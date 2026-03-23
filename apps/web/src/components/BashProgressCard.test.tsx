import {
  type CodeRenderContextValue,
  CodeRenderProvider,
} from '@claude-view/shared/contexts/CodeRenderContext'
import { fireEvent, render, screen } from '@testing-library/react'
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

const baseProps = {
  output: 'Compiling...\nDone.',
  fullOutput: 'Downloading deps...\nCompiling...\nDone.',
  elapsedTimeSeconds: 5.2,
  totalLines: 3,
  totalBytes: 256,
}

describe('BashProgressCard', () => {
  describe('Stats bar rendering', () => {
    it('should display elapsed time, line count, and byte size', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} />)

      expect(screen.getByText('5.2s')).toBeInTheDocument()
      expect(screen.getByText('3 lines')).toBeInTheDocument()
      expect(screen.getByText('256 B')).toBeInTheDocument()
    })

    it('should format bytes as KB when >= 1024', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} totalBytes={2048} />)

      expect(screen.getByText('2.0 KB')).toBeInTheDocument()
    })

    it('should format bytes as MB when >= 1MB', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} totalBytes={1048576} />)

      expect(screen.getByText('1.0 MB')).toBeInTheDocument()
    })

    it('should show singular "line" for 1 line', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} totalLines={1} />)

      expect(screen.getByText('1 line')).toBeInTheDocument()
    })

    it('should show taskId chip when provided', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} taskId="bg_001" />)

      expect(screen.getByText('task:bg_001')).toBeInTheDocument()
    })

    it('should not show taskId chip when null', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} taskId={null} />)

      expect(screen.queryByText(/task:/)).not.toBeInTheDocument()
    })
  })

  describe('Output rendering', () => {
    it('should show recent output by default', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} />)

      const codeBlock = screen.getByTestId('compact-code-block')
      expect(codeBlock.textContent).toContain('Compiling...')
      expect(codeBlock.textContent).toContain('Done.')
    })

    it('should render output via CompactCodeBlock with bash language', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} />)

      const codeBlock = screen.getByTestId('compact-code-block')
      expect(codeBlock).toHaveAttribute('data-language', 'bash')
    })

    it('should show "No output" when output is empty', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} output="" fullOutput="" />)

      expect(screen.getByText('No output')).toBeInTheDocument()
    })
  })

  describe('Expand toggle', () => {
    it('should show expand button when fullOutput is longer than output', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} />)

      expect(screen.getByText(/full output/)).toBeInTheDocument()
    })

    it('should not show expand button when output equals fullOutput', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} output="Same" fullOutput="Same" />)

      expect(screen.queryByText(/full output/)).not.toBeInTheDocument()
    })

    it('should switch to full output on click', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} />)

      fireEvent.click(screen.getByText(/full output/))

      const codeBlock = screen.getByTestId('compact-code-block')
      expect(codeBlock.textContent).toContain('Downloading deps...')
      expect(codeBlock.textContent).toContain('Compiling...')
    })
  })

  describe('Visual styling', () => {
    it('should have gray left border', () => {
      const { container } = renderWithCodeContext(<BashProgressCard {...baseProps} />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-gray')
    })

    it('should have bash-progress-card test id', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} />)

      expect(screen.getByTestId('bash-progress-card')).toBeInTheDocument()
    })
  })

  describe('BigInt handling', () => {
    it('should handle bigint totalBytes from Rust u64', () => {
      renderWithCodeContext(<BashProgressCard {...baseProps} totalBytes={BigInt(4096)} />)

      expect(screen.getByText('4.0 KB')).toBeInTheDocument()
    })
  })
})
