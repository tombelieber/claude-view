import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { GridControls, type GridControlsProps } from './GridControls'

function renderGridControls(overrides: Partial<GridControlsProps> = {}) {
  const defaultProps: GridControlsProps = {
    gridOverride: { cols: 2, rows: 2 },
    compactHeaders: false,
    verboseMode: false,
    sessionCount: 6,
    visibleCount: 6,
    onGridOverrideChange: vi.fn(),
    onCompactHeadersChange: vi.fn(),
    onVerboseModeChange: vi.fn(),
  }

  const props = { ...defaultProps, ...overrides }
  return { ...render(<GridControls {...props} />), props }
}

describe('GridControls', () => {
  describe('rendering', () => {
    it('renders cols slider', () => {
      renderGridControls()

      expect(screen.getByLabelText('Cols')).toBeInTheDocument()
    })

    it('renders rows slider', () => {
      renderGridControls()

      expect(screen.getByLabelText('Rows')).toBeInTheDocument()
    })

    it('renders Auto button', () => {
      renderGridControls()

      expect(screen.getByText('Auto')).toBeInTheDocument()
    })

    it('renders session count badge with total when all visible', () => {
      renderGridControls({ sessionCount: 4, visibleCount: 4 })

      expect(screen.getByText('4')).toBeInTheDocument()
      expect(screen.getByText('sessions')).toBeInTheDocument()
    })

    it('renders "session" (singular) when count is 1', () => {
      renderGridControls({ sessionCount: 1, visibleCount: 1 })

      expect(screen.getByText('session')).toBeInTheDocument()
    })

    it('renders "N of M sessions" when some are hidden', () => {
      renderGridControls({ sessionCount: 8, visibleCount: 5 })

      expect(screen.getByText('5')).toBeInTheDocument()
      expect(screen.getByText('of')).toBeInTheDocument()
      expect(screen.getByText('8')).toBeInTheDocument()
      expect(screen.getByText('sessions')).toBeInTheDocument()
    })

    it('renders Compact button', () => {
      renderGridControls()

      expect(screen.getByText('Compact')).toBeInTheDocument()
    })
  })

  describe('slider values', () => {
    it('displays current cols value', () => {
      renderGridControls({ gridOverride: { cols: 3, rows: 2 } })

      const colsSlider = screen.getByLabelText('Cols') as HTMLInputElement
      expect(colsSlider.value).toBe('3')
    })

    it('displays current rows value', () => {
      renderGridControls({ gridOverride: { cols: 2, rows: 4 } })

      const rowsSlider = screen.getByLabelText('Rows') as HTMLInputElement
      expect(rowsSlider.value).toBe('4')
    })

    it('displays dash when in auto mode', () => {
      renderGridControls({ gridOverride: null })

      // Sliders show '-' as the value text when in auto mode
      const dashes = screen.getAllByText('-')
      expect(dashes.length).toBe(2) // one for cols, one for rows
    })

    it('disables sliders in auto mode', () => {
      renderGridControls({ gridOverride: null })

      const colsSlider = screen.getByLabelText('Cols') as HTMLInputElement
      const rowsSlider = screen.getByLabelText('Rows') as HTMLInputElement
      expect(colsSlider.disabled).toBe(true)
      expect(rowsSlider.disabled).toBe(true)
    })
  })

  describe('interactions', () => {
    it('calls onGridOverrideChange when cols slider changes', () => {
      const onGridOverrideChange = vi.fn()
      renderGridControls({
        gridOverride: { cols: 2, rows: 2 },
        onGridOverrideChange,
      })

      const colsSlider = screen.getByLabelText('Cols')
      fireEvent.change(colsSlider, { target: { value: '3' } })

      expect(onGridOverrideChange).toHaveBeenCalledWith({ cols: 3, rows: 2 })
    })

    it('calls onGridOverrideChange when rows slider changes', () => {
      const onGridOverrideChange = vi.fn()
      renderGridControls({
        gridOverride: { cols: 2, rows: 2 },
        onGridOverrideChange,
      })

      const rowsSlider = screen.getByLabelText('Rows')
      fireEvent.change(rowsSlider, { target: { value: '4' } })

      expect(onGridOverrideChange).toHaveBeenCalledWith({ cols: 2, rows: 4 })
    })

    it('calls onGridOverrideChange(null) when Auto is clicked', () => {
      const onGridOverrideChange = vi.fn()
      renderGridControls({
        gridOverride: { cols: 3, rows: 3 },
        onGridOverrideChange,
      })

      const autoBtn = screen.getByText('Auto')
      fireEvent.click(autoBtn)

      expect(onGridOverrideChange).toHaveBeenCalledWith(null)
    })

    it('calls onCompactHeadersChange when Compact toggle is clicked', () => {
      const onCompactHeadersChange = vi.fn()
      renderGridControls({
        compactHeaders: false,
        onCompactHeadersChange,
      })

      const compactBtn = screen.getByText('Compact')
      fireEvent.click(compactBtn)

      expect(onCompactHeadersChange).toHaveBeenCalledWith(true)
    })

    it('calls onCompactHeadersChange(false) when compact is already on', () => {
      const onCompactHeadersChange = vi.fn()
      renderGridControls({
        compactHeaders: true,
        onCompactHeadersChange,
      })

      const compactBtn = screen.getByText('Compact')
      fireEvent.click(compactBtn)

      expect(onCompactHeadersChange).toHaveBeenCalledWith(false)
    })
  })

  describe('Auto button state', () => {
    it('shows Auto as active when gridOverride is null', () => {
      renderGridControls({ gridOverride: null })

      const autoBtn = screen.getByText('Auto').closest('button')!
      expect(autoBtn.getAttribute('aria-pressed')).toBe('true')
    })

    it('shows Auto as inactive when gridOverride is set', () => {
      renderGridControls({ gridOverride: { cols: 2, rows: 2 } })

      const autoBtn = screen.getByText('Auto').closest('button')!
      expect(autoBtn.getAttribute('aria-pressed')).toBe('false')
    })
  })

  describe('Compact button state', () => {
    it('shows Compact as active when compactHeaders is true', () => {
      renderGridControls({ compactHeaders: true })

      const compactBtn = screen.getByText('Compact').closest('button')!
      expect(compactBtn.getAttribute('aria-pressed')).toBe('true')
    })

    it('shows Compact as inactive when compactHeaders is false', () => {
      renderGridControls({ compactHeaders: false })

      const compactBtn = screen.getByText('Compact').closest('button')!
      expect(compactBtn.getAttribute('aria-pressed')).toBe('false')
    })
  })

  describe('Verbose toggle', () => {
    it('renders "Chat" label when verboseMode is off', () => {
      renderGridControls({ verboseMode: false })

      expect(screen.getByText('Chat')).toBeInTheDocument()
    })

    it('renders "Verbose" label when verboseMode is on', () => {
      renderGridControls({ verboseMode: true })

      expect(screen.getByText('Verbose')).toBeInTheDocument()
    })

    it('shows verbose button as active when verboseMode is true', () => {
      renderGridControls({ verboseMode: true })

      const verboseBtn = screen.getByText('Verbose').closest('button')!
      expect(verboseBtn.getAttribute('aria-pressed')).toBe('true')
    })

    it('shows verbose button as inactive when verboseMode is false', () => {
      renderGridControls({ verboseMode: false })

      const verboseBtn = screen.getByText('Chat').closest('button')!
      expect(verboseBtn.getAttribute('aria-pressed')).toBe('false')
    })

    it('calls onVerboseModeChange when clicked', () => {
      const onVerboseModeChange = vi.fn()
      renderGridControls({ verboseMode: false, onVerboseModeChange })

      const chatBtn = screen.getByText('Chat')
      fireEvent.click(chatBtn)

      expect(onVerboseModeChange).toHaveBeenCalledOnce()
    })
  })
})
