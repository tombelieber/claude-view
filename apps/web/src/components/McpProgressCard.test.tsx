import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { McpProgressCard } from './McpProgressCard'

vi.mock('./live/CompactCodeBlock', () => ({
  CompactCodeBlock: ({ code, language, blockId }: { code: string; language: string; blockId?: string }) => (
    <pre data-testid="compact-code-block" data-language={language} data-block-id={blockId}>{code}</pre>
  ),
}))

describe('McpProgressCard', () => {
  describe('Title and status rendering', () => {
    it('should display server.method with params', () => {
      render(
        <McpProgressCard
          server="filesystem"
          method="readFile"
          params={{ path: '/tmp/test.txt' }}
        />
      )

      expect(screen.getByText(/filesystem\.readFile/)).toBeInTheDocument()
    })

    it('should show "(no params)" when params is undefined', () => {
      render(
        <McpProgressCard server="memory" method="search" />
      )

      expect(screen.getByText(/\(no params\)/)).toBeInTheDocument()
    })

    it('should have purple left border', () => {
      const { container } = render(
        <McpProgressCard server="fs" method="read" />
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-purple')
    })
  })

  describe('Collapsible behavior', () => {
    it('should expand to show result on click', () => {
      render(
        <McpProgressCard
          server="filesystem"
          method="readFile"
          params={{ path: '/tmp/test.txt' }}
          result={{ content: 'file contents here' }}
        />
      )

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText(/file contents here/)).toBeInTheDocument()
    })

    it('should show params in expanded view', () => {
      render(
        <McpProgressCard
          server="fs"
          method="read"
          params={{ path: '/tmp/test.txt' }}
        />
      )

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText(/\/tmp\/test\.txt/)).toBeInTheDocument()
    })
  })

  describe('ARIA and keyboard', () => {
    it('should have ARIA label', () => {
      render(<McpProgressCard server="fs" method="read" />)
      expect(screen.getByRole('button', { name: /mcp/i })).toBeInTheDocument()
    })
  })
})
