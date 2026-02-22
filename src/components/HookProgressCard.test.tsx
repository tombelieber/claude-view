import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { HookProgressCard } from './HookProgressCard'

vi.mock('./live/CompactCodeBlock', () => ({
  CompactCodeBlock: ({ code, language, blockId }: { code: string; language: string; blockId?: string }) => (
    <pre data-testid="compact-code-block" data-language={language} data-block-id={blockId}>{code}</pre>
  ),
}))

describe('HookProgressCard', () => {
  describe('Title and status rendering', () => {
    it('should display hook event and command name', () => {
      render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup-env.sh"
        />
      )

      expect(screen.getByText(/SessionStart/)).toBeInTheDocument()
      expect(screen.getByText(/setup-env\.sh/)).toBeInTheDocument()
    })

    it('should have amber left border', () => {
      const { container } = render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="cmd"
        />
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-amber')
    })
  })

  describe('Output rendering', () => {
    it('should show output immediately via CompactCodeBlock', () => {
      render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup.sh"
          output="Environment configured successfully"
        />
      )

      expect(screen.getByText(/Environment configured successfully/)).toBeInTheDocument()
    })

    it('should render output via CompactCodeBlock with bash language', () => {
      render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup.sh"
          output="some output"
        />
      )

      const codeBlock = screen.getByTestId('compact-code-block')
      expect(codeBlock).toHaveAttribute('data-language', 'bash')
      expect(codeBlock).toHaveTextContent('some output')
    })

    it('should not render code block when output is undefined', () => {
      render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup.sh"
        />
      )

      expect(screen.queryByTestId('compact-code-block')).not.toBeInTheDocument()
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="cmd"
        />
      )
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
