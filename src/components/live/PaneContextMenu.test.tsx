import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { PaneContextMenu, type PaneContextMenuProps } from './PaneContextMenu'

function renderContextMenu(overrides: Partial<PaneContextMenuProps> = {}) {
  const defaultProps: PaneContextMenuProps = {
    x: 100,
    y: 200,
    sessionId: 'session-1',
    isPinned: false,
    mode: 'raw',
    onClose: vi.fn(),
    onPin: vi.fn(),
    onUnpin: vi.fn(),
    onHide: vi.fn(),
    onMoveToFront: vi.fn(),
    onExpand: vi.fn(),
    onToggleMode: vi.fn(),
  }

  const props = { ...defaultProps, ...overrides }
  return { ...render(<PaneContextMenu {...props} />), props }
}

describe('PaneContextMenu', () => {
  describe('menu items rendering', () => {
    it('renders all 5 menu items when not pinned', () => {
      renderContextMenu({ isPinned: false })

      const menuItems = screen.getAllByRole('menuitem')
      expect(menuItems).toHaveLength(5)
    })

    it('renders Pin pane item when isPinned=false', () => {
      renderContextMenu({ isPinned: false })

      expect(screen.getByText('Pin pane')).toBeInTheDocument()
      expect(screen.queryByText('Unpin pane')).not.toBeInTheDocument()
    })

    it('renders Unpin pane item when isPinned=true', () => {
      renderContextMenu({ isPinned: true })

      expect(screen.getByText('Unpin pane')).toBeInTheDocument()
      expect(screen.queryByText('Pin pane')).not.toBeInTheDocument()
    })

    it('renders Hide pane item', () => {
      renderContextMenu()

      expect(screen.getByText('Hide pane')).toBeInTheDocument()
    })

    it('renders Move to front item', () => {
      renderContextMenu()

      expect(screen.getByText('Move to front')).toBeInTheDocument()
    })

    it('renders Expand item', () => {
      renderContextMenu()

      expect(screen.getByText('Expand')).toBeInTheDocument()
    })

    it('renders "Switch to Rich" when mode is raw', () => {
      renderContextMenu({ mode: 'raw' })

      expect(screen.getByText('Switch to Rich')).toBeInTheDocument()
      expect(screen.queryByText('Switch to Raw')).not.toBeInTheDocument()
    })

    it('renders "Switch to Raw" when mode is rich', () => {
      renderContextMenu({ mode: 'rich' })

      expect(screen.getByText('Switch to Raw')).toBeInTheDocument()
      expect(screen.queryByText('Switch to Rich')).not.toBeInTheDocument()
    })
  })

  describe('menu item actions', () => {
    it('calls onPin and onClose when Pin pane is clicked', () => {
      const onPin = vi.fn()
      const onClose = vi.fn()
      renderContextMenu({ isPinned: false, onPin, onClose })

      fireEvent.click(screen.getByText('Pin pane'))

      expect(onPin).toHaveBeenCalledTimes(1)
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('calls onUnpin and onClose when Unpin pane is clicked', () => {
      const onUnpin = vi.fn()
      const onClose = vi.fn()
      renderContextMenu({ isPinned: true, onUnpin, onClose })

      fireEvent.click(screen.getByText('Unpin pane'))

      expect(onUnpin).toHaveBeenCalledTimes(1)
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('calls onHide and onClose when Hide pane is clicked', () => {
      const onHide = vi.fn()
      const onClose = vi.fn()
      renderContextMenu({ onHide, onClose })

      fireEvent.click(screen.getByText('Hide pane'))

      expect(onHide).toHaveBeenCalledTimes(1)
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('calls onMoveToFront and onClose when Move to front is clicked', () => {
      const onMoveToFront = vi.fn()
      const onClose = vi.fn()
      renderContextMenu({ onMoveToFront, onClose })

      fireEvent.click(screen.getByText('Move to front'))

      expect(onMoveToFront).toHaveBeenCalledTimes(1)
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('calls onExpand and onClose when Expand is clicked', () => {
      const onExpand = vi.fn()
      const onClose = vi.fn()
      renderContextMenu({ onExpand, onClose })

      fireEvent.click(screen.getByText('Expand'))

      expect(onExpand).toHaveBeenCalledTimes(1)
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('calls onToggleMode and onClose when Switch to Rich is clicked', () => {
      const onToggleMode = vi.fn()
      const onClose = vi.fn()
      renderContextMenu({ mode: 'raw', onToggleMode, onClose })

      fireEvent.click(screen.getByText('Switch to Rich'))

      expect(onToggleMode).toHaveBeenCalledTimes(1)
      expect(onClose).toHaveBeenCalledTimes(1)
    })
  })

  describe('close behavior', () => {
    it('calls onClose when clicking outside the menu', () => {
      const onClose = vi.fn()
      renderContextMenu({ onClose })

      // Simulate mousedown outside the menu
      fireEvent.mouseDown(document.body)

      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('calls onClose when Escape is pressed', () => {
      const onClose = vi.fn()
      renderContextMenu({ onClose })

      fireEvent.keyDown(document, { key: 'Escape' })

      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('does not call onClose when clicking inside the menu', () => {
      const onClose = vi.fn()
      renderContextMenu({ onClose })

      const menu = screen.getByRole('menu')
      fireEvent.mouseDown(menu)

      expect(onClose).not.toHaveBeenCalled()
    })
  })

  describe('ARIA attributes', () => {
    it('has role="menu" on the container', () => {
      renderContextMenu()

      expect(screen.getByRole('menu')).toBeInTheDocument()
    })

    it('has role="menuitem" on each item', () => {
      renderContextMenu()

      const items = screen.getAllByRole('menuitem')
      expect(items).toHaveLength(5)
    })

    it('has aria-orientation="vertical"', () => {
      renderContextMenu()

      const menu = screen.getByRole('menu')
      expect(menu.getAttribute('aria-orientation')).toBe('vertical')
    })
  })
})
