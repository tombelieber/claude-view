import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { HookProgressCard } from './HookProgressCard'

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

      expect(screen.getByText(/Hook: SessionStart/)).toBeInTheDocument()
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

  describe('Collapsible behavior', () => {
    it('should expand to show output on click when output exists', () => {
      render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup.sh"
          output="Environment configured successfully"
        />
      )

      expect(screen.queryByText(/Environment configured/)).not.toBeInTheDocument()

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText(/Environment configured successfully/)).toBeInTheDocument()
    })

    it('should not be expandable when output is undefined', () => {
      render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="setup.sh"
        />
      )

      // No chevron/expand indicator when no output
      expect(screen.queryByTestId('hook-expand-icon')).not.toBeInTheDocument()
    })
  })

  describe('ARIA and keyboard', () => {
    it('should have ARIA label', () => {
      render(
        <HookProgressCard
          hookEvent="SessionStart"
          hookName="pre-session"
          command="cmd"
          output="some output"
        />
      )

      expect(screen.getByRole('button', { name: /hook/i })).toBeInTheDocument()
    })
  })
})
