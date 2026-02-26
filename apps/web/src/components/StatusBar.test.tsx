import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { StatusBar } from './StatusBar'
import type { ProjectSummary } from '../hooks/use-projects'

// Mock sonner
const mockToastSuccess = vi.fn()
const mockToastError = vi.fn()
const mockToastInfo = vi.fn()
const mockToastWarning = vi.fn()

vi.mock('sonner', () => ({
  toast: {
    success: (msg: string, opts?: unknown) => mockToastSuccess(msg, opts),
    error: (msg: string, opts?: unknown) => mockToastError(msg, opts),
    info: (msg: string, opts?: unknown) => mockToastInfo(msg, opts),
    warning: (msg: string, opts?: unknown) => mockToastWarning(msg, opts),
  },
  Toaster: () => null,
}))

// Mock use-status hook
const mockUseStatus = vi.fn()
vi.mock('../hooks/use-status', () => ({
  useStatus: () => mockUseStatus(),
  formatRelativeTime: (ts: bigint | null) => ts ? '5 minutes ago' : null,
  useTick: () => 0,
}))

// Mock use-git-sync hook
const mockTriggerSync = vi.fn()
const mockResetSync = vi.fn()
const mockUseGitSync = vi.fn()
vi.mock('../hooks/use-git-sync', () => ({
  useGitSync: () => mockUseGitSync(),
}))

// Mock use-git-sync-progress hook
const mockUseGitSyncProgress = vi.fn()
vi.mock('../hooks/use-git-sync-progress', () => ({
  useGitSyncProgress: (enabled: boolean) => mockUseGitSyncProgress(enabled),
}))

// Mock fetch for polling
const mockFetch = vi.fn()

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  })

  return function Wrapper({ children }: { children: React.ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        {children}
      </QueryClientProvider>
    )
  }
}

const mockProjects: ProjectSummary[] = [
  {
    projectPath: '/project1',
    sessionCount: 10,
    latestTs: BigInt(Math.floor(Date.now() / 1000)),
  },
  {
    projectPath: '/project2',
    sessionCount: 5,
    latestTs: BigInt(Math.floor(Date.now() / 1000)),
  },
]

describe('StatusBar', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    global.fetch = mockFetch

    // Default mock implementations
    mockUseStatus.mockReturnValue({
      data: {
        sessionsIndexed: BigInt(15),
        lastIndexedAt: BigInt(Math.floor(Date.now() / 1000)),
        lastGitSyncAt: BigInt(Math.floor(Date.now() / 1000)),
        commitsFound: BigInt(10),
        linksCreated: BigInt(5),
      },
      isLoading: false,
    })

    mockUseGitSync.mockReturnValue({
      triggerSync: mockTriggerSync,
      status: 'idle',
      isLoading: false,
      error: null,
      response: null,
      reset: mockResetSync,
    })

    mockUseGitSyncProgress.mockReturnValue({
      phase: 'idle',
      reposScanned: 0,
      totalRepos: 0,
      commitsFound: 0,
      sessionsCorrelated: 0,
      totalCorrelatableSessions: 0,
      linksCreated: 0,
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('AC-3.1: Button shows "Sync Now" with refresh icon', () => {
    it('should display labeled "Sync Now" button', () => {
      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      expect(button).toBeInTheDocument()
      expect(button).toHaveTextContent('Sync Now')
    })

    it('should have a refresh icon in the button', () => {
      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      // Check that the button contains an SVG (the RefreshCw icon)
      const svg = button.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })
  })

  describe('AC-3.2: Click shows "Syncing..." with spinning icon', () => {
    it('should show "Syncing..." text when sync is in progress', () => {
      mockUseGitSync.mockReturnValue({
        triggerSync: mockTriggerSync,
        status: 'running',
        isLoading: true,
        error: null,
        response: null,
        reset: mockResetSync,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      expect(button).toHaveTextContent('Syncing...')
    })

    it('should disable button while syncing', () => {
      mockUseGitSync.mockReturnValue({
        triggerSync: mockTriggerSync,
        status: 'running',
        isLoading: true,
        error: null,
        response: null,
        reset: mockResetSync,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      expect(button).toBeDisabled()
    })

    it('should apply spinning animation to icon while syncing', () => {
      mockUseGitSync.mockReturnValue({
        triggerSync: mockTriggerSync,
        status: 'running',
        isLoading: true,
        error: null,
        response: null,
        reset: mockResetSync,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      const svg = button.querySelector('svg')
      expect(svg).toHaveClass('animate-spin')
    })
  })

  describe('AC-3.3/3.4/3.5: Success toast with stats, auto-dismiss', () => {
    it('should trigger sync when button is clicked', async () => {
      mockTriggerSync.mockResolvedValue(true)

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      fireEvent.click(button)

      await waitFor(() => {
        expect(mockTriggerSync).toHaveBeenCalled()
      })
    })

    it('should not trigger sync if already syncing', async () => {
      mockUseGitSync.mockReturnValue({
        triggerSync: mockTriggerSync,
        status: 'running',
        isLoading: true,
        error: null,
        response: null,
        reset: mockResetSync,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      fireEvent.click(button)

      // Should not call triggerSync because button is disabled and handler checks isSpinning
      expect(mockTriggerSync).not.toHaveBeenCalled()
    })
  })

  describe('AC-3.6/3.7: Error toast with retry button', () => {
    it('should show error toast when sync fails', async () => {
      // Error toasts are driven by SSE progress phase, not useGitSync status
      mockUseGitSyncProgress.mockReturnValue({
        phase: 'error',
        reposScanned: 0,
        totalRepos: 0,
        commitsFound: 0,
        sessionsCorrelated: 0,
        totalCorrelatableSessions: 0,
        linksCreated: 0,
        errorMessage: 'Network error',
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalledWith('Sync failed', expect.objectContaining({
          description: 'Network error',
          duration: 6000,
          action: expect.objectContaining({
            label: 'Retry',
          }),
        }))
      })
    })

    it('should not show duplicate error toasts', async () => {
      mockUseGitSyncProgress.mockReturnValue({
        phase: 'error',
        reposScanned: 0,
        totalRepos: 0,
        commitsFound: 0,
        sessionsCorrelated: 0,
        totalCorrelatableSessions: 0,
        linksCreated: 0,
        errorMessage: 'Network error',
      })

      const { rerender } = render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalledTimes(1)
      })

      // Re-render with same error
      rerender(<StatusBar projects={mockProjects} />)

      // Should still only have one call (deduplication via doneHandledRef)
      expect(mockToastError).toHaveBeenCalledTimes(1)
    })
  })

  describe('AC-10.1: Concurrent click protection', () => {
    it('should ignore second click when button is disabled during sync', async () => {
      mockUseGitSync.mockReturnValue({
        triggerSync: mockTriggerSync,
        status: 'running',
        isLoading: true,
        error: null,
        response: null,
        reset: mockResetSync,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')

      // Try to click the disabled button
      fireEvent.click(button)
      fireEvent.click(button)

      // Button is disabled, so handler's isSpinning check prevents execution
      expect(mockTriggerSync).not.toHaveBeenCalled()
    })
  })

  describe('AC-10.2: Conflict handling (409)', () => {
    it('should show info toast when sync returns 409 conflict', async () => {
      mockUseGitSync.mockReturnValue({
        triggerSync: mockTriggerSync,
        status: 'conflict',
        isLoading: false,
        error: null,
        response: null,
        reset: mockResetSync,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(mockToastInfo).toHaveBeenCalledWith('Sync already in progress', expect.objectContaining({
          description: 'Please wait for the current sync to complete.',
          duration: 3000,
        }))
      })
    })
  })

  describe('Status display', () => {
    it('should show loading state', () => {
      mockUseStatus.mockReturnValue({
        data: null,
        isLoading: true,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      expect(screen.getByText('Loading status...')).toBeInTheDocument()
    })

    it('should show session count and last update time', () => {
      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      expect(screen.getByText('Last update: 5 minutes ago')).toBeInTheDocument()
      expect(screen.getByText('15 sessions')).toBeInTheDocument()
    })

    it('should show commit count when available', () => {
      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      expect(screen.getByText('10')).toBeInTheDocument()
    })

    it('should show "Not yet synced" when no sync has occurred', () => {
      mockUseStatus.mockReturnValue({
        data: null,
        isLoading: false,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      expect(screen.getByText(/Not yet synced/)).toBeInTheDocument()
    })
  })

  describe('Accessibility', () => {
    it('should have proper ARIA labels', () => {
      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      expect(button).toHaveAttribute('aria-label', 'Sync now')

      const footer = screen.getByRole('contentinfo')
      expect(footer).toHaveAttribute('aria-label', 'Data freshness status')
    })

    it('should update aria-label when syncing', () => {
      mockUseGitSync.mockReturnValue({
        triggerSync: mockTriggerSync,
        status: 'running',
        isLoading: true,
        error: null,
        response: null,
        reset: mockResetSync,
      })

      render(<StatusBar projects={mockProjects} />, { wrapper: createWrapper() })

      const button = screen.getByTestId('sync-button')
      expect(button).toHaveAttribute('aria-label', 'Sync in progress')
    })
  })
})
