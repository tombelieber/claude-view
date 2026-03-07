import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Banner } from './Banner'

// Mock localStorage using vi.spyOn (matches codebase convention in use-layout-mode.test.ts)
const mockStorage = new Map<string, string>()

beforeEach(() => {
  mockStorage.clear()
  vi.spyOn(Storage.prototype, 'getItem').mockImplementation((key) => mockStorage.get(key) ?? null)
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation((key, value) => {
    mockStorage.set(key, value)
  })
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('Banner', () => {
  describe('variants', () => {
    it('renders error variant with AlertCircle icon', () => {
      render(<Banner variant="error">Something broke</Banner>)
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByText('Something broke')).toBeInTheDocument()
    })

    it('renders warning variant', () => {
      render(<Banner variant="warning">Heads up</Banner>)
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByText('Heads up')).toBeInTheDocument()
    })

    it('renders info variant', () => {
      render(<Banner variant="info">FYI</Banner>)
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByText('FYI')).toBeInTheDocument()
    })

    it('renders experimental variant with FlaskConical icon', () => {
      render(<Banner variant="experimental">Beta feature</Banner>)
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByText('Beta feature')).toBeInTheDocument()
    })
  })

  describe('layouts', () => {
    it('bar layout has no border-radius', () => {
      const { container } = render(
        <Banner variant="warning" layout="bar">
          Bar
        </Banner>,
      )
      const el = container.firstChild as HTMLElement
      expect(el.className).toMatch(/border-b/)
      expect(el.className).not.toMatch(/rounded/)
    })

    it('inline layout (default) has rounded corners', () => {
      const { container } = render(<Banner variant="warning">Inline</Banner>)
      const el = container.firstChild as HTMLElement
      expect(el.className).toMatch(/rounded-xl/)
    })
  })

  describe('dismiss', () => {
    it('shows dismiss button when dismissKey is provided', () => {
      render(
        <Banner variant="info" dismissKey="test-dismiss">
          Dismissible
        </Banner>,
      )
      expect(screen.getByLabelText('Dismiss')).toBeInTheDocument()
    })

    it('hides dismiss button when no dismissKey', () => {
      render(<Banner variant="info">Persistent</Banner>)
      expect(screen.queryByLabelText('Dismiss')).not.toBeInTheDocument()
    })

    it('dismisses on click and persists to localStorage', async () => {
      const user = userEvent.setup()
      render(
        <Banner variant="info" dismissKey="test-key">
          Bye
        </Banner>,
      )
      await user.click(screen.getByLabelText('Dismiss'))
      expect(screen.queryByText('Bye')).not.toBeInTheDocument()
      expect(Storage.prototype.setItem).toHaveBeenCalledWith('test-key', 'true')
    })

    it('returns null if already dismissed in localStorage', () => {
      mockStorage.set('test-key', 'true')
      const { container } = render(
        <Banner variant="info" dismissKey="test-key">
          Gone
        </Banner>,
      )
      expect(container.firstChild).toBeNull()
    })
  })

  describe('action', () => {
    it('renders action button when provided', () => {
      const onClick = vi.fn()
      render(
        <Banner variant="warning" action={{ label: 'Sync', onClick }}>
          Data stale
        </Banner>,
      )
      expect(screen.getByRole('button', { name: 'Sync' })).toBeInTheDocument()
    })

    it('calls action onClick when clicked', async () => {
      const user = userEvent.setup()
      const onClick = vi.fn()
      render(
        <Banner variant="warning" action={{ label: 'Retry', onClick }}>
          Failed
        </Banner>,
      )
      await user.click(screen.getByRole('button', { name: 'Retry' }))
      expect(onClick).toHaveBeenCalledOnce()
    })
  })

  describe('className', () => {
    it('merges custom className', () => {
      const { container } = render(
        <Banner variant="info" className="mt-4">
          Content
        </Banner>,
      )
      expect(container.firstChild).toHaveClass('mt-4')
    })
  })
})
