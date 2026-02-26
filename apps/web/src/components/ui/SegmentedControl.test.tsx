import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SegmentedControl } from './SegmentedControl'

type TimeRange = '7d' | '30d' | '90d' | 'all'

const defaultOptions = [
  { value: '7d' as TimeRange, label: '7 days' },
  { value: '30d' as TimeRange, label: '30 days' },
  { value: '90d' as TimeRange, label: '90 days' },
  { value: 'all' as TimeRange, label: 'All time' },
]

describe('SegmentedControl', () => {
  describe('rendering', () => {
    it('renders all options', () => {
      render(
        <SegmentedControl
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      expect(screen.getByRole('radio', { name: '7 days' })).toBeInTheDocument()
      expect(screen.getByRole('radio', { name: '30 days' })).toBeInTheDocument()
      expect(screen.getByRole('radio', { name: '90 days' })).toBeInTheDocument()
      expect(screen.getByRole('radio', { name: 'All time' })).toBeInTheDocument()
    })

    it('marks the selected option as checked', () => {
      render(
        <SegmentedControl
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      expect(screen.getByRole('radio', { name: '30 days' })).toHaveAttribute(
        'aria-checked',
        'true'
      )
      expect(screen.getByRole('radio', { name: '7 days' })).toHaveAttribute(
        'aria-checked',
        'false'
      )
    })

    it('renders with custom aria label', () => {
      render(
        <SegmentedControl
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
          ariaLabel="Select time period"
        />
      )

      expect(
        screen.getByRole('radiogroup', { name: 'Select time period' })
      ).toBeInTheDocument()
    })
  })

  describe('interaction', () => {
    it('calls onChange when option is clicked', async () => {
      const user = userEvent.setup()
      const onChange = vi.fn()

      render(
        <SegmentedControl
          value="30d"
          onChange={onChange}
          options={defaultOptions}
        />
      )

      await user.click(screen.getByRole('radio', { name: '7 days' }))
      expect(onChange).toHaveBeenCalledWith('7d')
    })

    it('calls onChange with correct value for each option', async () => {
      const user = userEvent.setup()
      const onChange = vi.fn()

      render(
        <SegmentedControl
          value="30d"
          onChange={onChange}
          options={defaultOptions}
        />
      )

      await user.click(screen.getByRole('radio', { name: '90 days' }))
      expect(onChange).toHaveBeenCalledWith('90d')

      await user.click(screen.getByRole('radio', { name: 'All time' }))
      expect(onChange).toHaveBeenCalledWith('all')
    })
  })

  describe('accessibility', () => {
    it('has radiogroup role on container', () => {
      render(
        <SegmentedControl
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      expect(screen.getByRole('radiogroup')).toBeInTheDocument()
    })

    it('each option has radio role', () => {
      render(
        <SegmentedControl
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      const radios = screen.getAllByRole('radio')
      expect(radios).toHaveLength(4)
    })

    it('selected option is visually distinct', () => {
      render(
        <SegmentedControl
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      const selected = screen.getByRole('radio', { name: '30 days' })
      const unselected = screen.getByRole('radio', { name: '7 days' })

      // Selected should have different styling (white background)
      expect(selected.className).toContain('bg-white')
      expect(unselected.className).not.toContain('bg-white')
    })
  })
})
