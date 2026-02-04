import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { StorageOverview } from './StorageOverview'

// Mock fetch globally
const mockFetch = vi.fn()
global.fetch = mockFetch

// Create a wrapper with QueryClient
function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  })
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }
}

describe('StorageOverview', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('loading state', () => {
    it('should show loading state while fetching', async () => {
      // Make fetch never resolve to keep loading state
      mockFetch.mockImplementation(() => new Promise(() => {}))

      render(<StorageOverview />, { wrapper: createWrapper() })

      expect(screen.getByText('Loading storage data...')).toBeInTheDocument()
    })
  })

  describe('error state', () => {
    it('should show error state when fetch fails', async () => {
      mockFetch.mockRejectedValueOnce(new Error('Network error'))

      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(screen.getByText('Failed to load storage data')).toBeInTheDocument()
      })
    })
  })

  describe('success state', () => {
    const mockStats = {
      jsonlBytes: BigInt(12000000000), // ~11.2 GB
      sqliteBytes: BigInt(256901120), // ~245 MB
      indexBytes: BigInt(134217728), // ~128 MB
      sessionCount: BigInt(6742),
      projectCount: BigInt(47),
      commitCount: BigInt(1245),
      oldestSessionDate: BigInt(1728864000), // Oct 14, 2024
      lastIndexAt: BigInt(Math.floor(Date.now() / 1000) - 2), // 2s ago
      lastIndexDurationMs: BigInt(3200),
      lastIndexSessionCount: BigInt(6742),
      lastGitSyncAt: BigInt(Math.floor(Date.now() / 1000) - 180), // 3m ago
      lastGitSyncDurationMs: null,
      lastGitSyncRepoCount: BigInt(0),
    }

    beforeEach(() => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve(mockStats),
      })
    })

    it('should render storage progress bars', async () => {
      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(screen.getByText('JSONL Sessions')).toBeInTheDocument()
        expect(screen.getByText('SQLite Database')).toBeInTheDocument()
        expect(screen.getByText('Search Index')).toBeInTheDocument()
      })
    })

    it('should render counts grid', async () => {
      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(screen.getByText('Sessions')).toBeInTheDocument()
        expect(screen.getByText('Projects')).toBeInTheDocument()
        expect(screen.getByText('Commits')).toBeInTheDocument()
        expect(screen.getByText('Oldest Session')).toBeInTheDocument()
        expect(screen.getByText('Index Built')).toBeInTheDocument()
        expect(screen.getByText('Last Git Sync')).toBeInTheDocument()
      })
    })

    it('should render action buttons', async () => {
      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(screen.getByText('Actions')).toBeInTheDocument()
        expect(screen.getByText('Rebuild Index')).toBeInTheDocument()
        expect(screen.getByText('Clear Cache')).toBeInTheDocument()
      })
    })

    it('should render index performance section', async () => {
      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        expect(screen.getByText('Index Performance')).toBeInTheDocument()
        expect(screen.getByText(/Last deep index:/)).toBeInTheDocument()
      })
    })

    it('should disable Clear Cache button', async () => {
      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        const clearCacheButton = screen.getByText('Clear Cache').closest('button')
        expect(clearCacheButton).toBeDisabled()
      })
    })
  })

  describe('empty state', () => {
    const emptyStats = {
      jsonlBytes: BigInt(0),
      sqliteBytes: BigInt(4096),
      indexBytes: BigInt(0),
      sessionCount: BigInt(0),
      projectCount: BigInt(0),
      commitCount: BigInt(0),
      oldestSessionDate: null,
      lastIndexAt: null,
      lastIndexDurationMs: null,
      lastIndexSessionCount: BigInt(0),
      lastGitSyncAt: null,
      lastGitSyncDurationMs: null,
      lastGitSyncRepoCount: BigInt(0),
    }

    beforeEach(() => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve(emptyStats),
      })
    })

    it('should render with zero counts', async () => {
      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        // Should show "0" for counts
        const zeroElements = screen.getAllByText('0')
        expect(zeroElements.length).toBeGreaterThan(0)
      })
    })

    it('should show Never for null timestamps', async () => {
      render(<StorageOverview />, { wrapper: createWrapper() })

      await waitFor(() => {
        const neverElements = screen.getAllByText('Never')
        expect(neverElements.length).toBeGreaterThan(0)
      })
    })
  })
})
