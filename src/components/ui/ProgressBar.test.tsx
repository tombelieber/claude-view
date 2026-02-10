import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ProgressBar } from './ProgressBar'

describe('ProgressBar', () => {
  describe('rendering', () => {
    it('should render label and percentage', () => {
      render(<ProgressBar label="Token Usage" value={75} max={100} />)

      expect(screen.getByText('Token Usage')).toBeInTheDocument()
      expect(screen.getByText('75%')).toBeInTheDocument()
    })

    it('should render suffix when provided', () => {
      render(
        <ProgressBar
          label="claude-opus-4"
          value={3500000}
          max={4800000}
          suffix="3.5M"
        />
      )

      expect(screen.getByText('3.5M')).toBeInTheDocument()
    })

    it('should render without suffix', () => {
      const { container } = render(
        <ProgressBar label="Progress" value={50} max={100} />
      )

      // Should not have extra elements for suffix
      const suffixElements = container.querySelectorAll('.font-medium')
      expect(suffixElements.length).toBe(1) // Only the label should have font-medium
    })
  })

  describe('percentage calculation', () => {
    it('should calculate correct percentage', () => {
      render(<ProgressBar label="Test" value={25} max={100} />)
      expect(screen.getByText('25%')).toBeInTheDocument()
    })

    it('should round percentage to nearest integer', () => {
      render(<ProgressBar label="Test" value={33} max={100} />)
      expect(screen.getByText('33%')).toBeInTheDocument()
    })

    it('should handle 0 max value without division error', () => {
      render(<ProgressBar label="Test" value={50} max={0} />)
      expect(screen.getByText('0%')).toBeInTheDocument()
    })

    it('should clamp percentage to 100% when value exceeds max', () => {
      render(<ProgressBar label="Test" value={150} max={100} />)
      expect(screen.getByText('100%')).toBeInTheDocument()
    })

    it('should handle negative values by clamping to 0%', () => {
      render(<ProgressBar label="Test" value={-10} max={100} />)
      expect(screen.getByText('0%')).toBeInTheDocument()
    })
  })

  describe('progress bar fill', () => {
    it('should set correct width style on progress fill', () => {
      const { container } = render(
        <ProgressBar label="Test" value={50} max={100} />
      )

      const progressFill = container.querySelector('[class*="bg-gradient"]')
      expect(progressFill).toHaveStyle({ width: '50%' })
    })

    it('should have gradient styling', () => {
      const { container } = render(
        <ProgressBar label="Test" value={50} max={100} />
      )

      const progressFill = container.querySelector('[class*="bg-gradient"]')
      expect(progressFill?.className).toMatch(/bg-gradient-to-r/)
      expect(progressFill?.className).toMatch(/from-blue-500/)
      expect(progressFill?.className).toMatch(/to-blue-800/)
    })
  })

  describe('accessibility', () => {
    it('should have progressbar role', () => {
      render(<ProgressBar label="Test" value={50} max={100} />)

      const progressbar = screen.getByRole('progressbar')
      expect(progressbar).toBeInTheDocument()
    })

    it('should have correct aria-valuenow', () => {
      render(<ProgressBar label="Test" value={75} max={100} />)

      const progressbar = screen.getByRole('progressbar')
      expect(progressbar).toHaveAttribute('aria-valuenow', '75')
    })

    it('should have correct aria-valuemin', () => {
      render(<ProgressBar label="Test" value={50} max={100} />)

      const progressbar = screen.getByRole('progressbar')
      expect(progressbar).toHaveAttribute('aria-valuemin', '0')
    })

    it('should have correct aria-valuemax', () => {
      render(<ProgressBar label="Test" value={50} max={100} />)

      const progressbar = screen.getByRole('progressbar')
      expect(progressbar).toHaveAttribute('aria-valuemax', '100')
    })

    it('should have aria-label with percentage', () => {
      render(<ProgressBar label="Token Usage" value={50} max={100} />)

      const progressbar = screen.getByRole('progressbar')
      expect(progressbar).toHaveAttribute(
        'aria-label',
        expect.stringContaining('Token Usage')
      )
      expect(progressbar).toHaveAttribute(
        'aria-label',
        expect.stringContaining('50%')
      )
    })

    it('should include suffix in aria-label when provided', () => {
      render(
        <ProgressBar
          label="Token Usage"
          value={50}
          max={100}
          suffix="500K"
        />
      )

      const progressbar = screen.getByRole('progressbar')
      expect(progressbar).toHaveAttribute(
        'aria-label',
        expect.stringContaining('500K')
      )
    })
  })

  describe('styling', () => {
    it('should apply custom className', () => {
      const { container } = render(
        <ProgressBar
          label="Test"
          value={50}
          max={100}
          className="custom-class"
        />
      )

      expect(container.firstChild).toHaveClass('custom-class')
    })

    it('should have tabular-nums for numerical display', () => {
      const { container } = render(
        <ProgressBar label="Test" value={50} max={100} />
      )

      const numericElements = container.querySelectorAll('.tabular-nums')
      expect(numericElements.length).toBeGreaterThan(0)
    })

    it('should have rounded corners on progress bar', () => {
      const { container } = render(
        <ProgressBar label="Test" value={50} max={100} />
      )

      const progressContainer = container.querySelector('[role="progressbar"]')
      expect(progressContainer?.className).toMatch(/rounded-full/)
    })
  })

  describe('edge cases', () => {
    it('should handle very large numbers', () => {
      render(
        <ProgressBar
          label="Tokens"
          value={4800000}
          max={10000000}
          suffix="4.8M"
        />
      )

      expect(screen.getByText('48%')).toBeInTheDocument()
      expect(screen.getByText('4.8M')).toBeInTheDocument()
    })

    it('should handle decimal values', () => {
      render(<ProgressBar label="Test" value={33.33} max={100} />)
      // Should round to nearest integer
      expect(screen.getByText('33%')).toBeInTheDocument()
    })

    it('should handle empty label', () => {
      render(<ProgressBar label="" value={50} max={100} />)

      const progressbar = screen.getByRole('progressbar')
      expect(progressbar).toBeInTheDocument()
    })
  })

  describe('stacked layout (mobile)', () => {
    it('should render label on its own line when stacked', () => {
      const { container } = render(
        <ProgressBar
          label="Token Usage"
          value={50}
          max={100}
          suffix="500K"
          stacked
        />
      )

      // In stacked mode, label should be block display
      const label = container.querySelector('.block')
      expect(label).toBeInTheDocument()
      expect(label).toHaveTextContent('Token Usage')
    })

    it('should render percentage and suffix below label when stacked', () => {
      render(
        <ProgressBar
          label="Token Usage"
          value={50}
          max={100}
          suffix="500K"
          stacked
        />
      )

      expect(screen.getByText('50%')).toBeInTheDocument()
      expect(screen.getByText('500K')).toBeInTheDocument()
    })

    it('should have proper spacing in stacked mode', () => {
      const { container } = render(
        <ProgressBar
          label="Test"
          value={50}
          max={100}
          stacked
        />
      )

      // Stacked layout has mb-3 class for more spacing
      expect(container.firstChild).toHaveClass('mb-3')
    })

    it('should default to non-stacked layout', () => {
      const { container } = render(
        <ProgressBar label="Test" value={50} max={100} />
      )

      // Default layout has mb-2 class
      expect(container.firstChild).toHaveClass('mb-2')
    })
  })
})
