import { describe, it, expect, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Sidebar } from './Sidebar'
import type { ProjectSummary } from '../types/generated/ProjectSummary'

const mockProjects: ProjectSummary[] = [
  { name: 'project-a', displayName: 'Project A', sessionCount: 10, path: '/path/a', activeCount: 2 },
  { name: 'project-b', displayName: 'Project B', sessionCount: 5, path: '/path/b', activeCount: 1 },
]

function createWrapper(initialUrl = '/') {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[initialUrl]}>
        {children}
      </MemoryRouter>
    </QueryClientProvider>
  )
}

// Mock the hooks that make API calls
vi.mock('../hooks/use-branches', () => ({
  useProjectBranches: () => ({
    data: { branches: [{ branch: 'main', count: BigInt(5) }, { branch: 'feature/auth', count: BigInt(3) }] },
    isLoading: false,
    error: null,
    refetch: vi.fn(),
  }),
}))

vi.mock('../hooks/use-recent-sessions', () => ({
  useRecentSessions: (project: string | null, _branch: string | null) => {
    if (!project) return { data: [], isLoading: false }
    return {
      data: [
        { id: 's1', preview: 'Fix auth token refresh', modifiedAt: Date.now() / 1000 - 7200 },
        { id: 's2', preview: 'Add unit tests', modifiedAt: Date.now() / 1000 - 18000 },
      ],
      isLoading: false,
    }
  },
}))

describe('Sidebar three-zone architecture', () => {
  // Zone 1 tests
  describe('Zone 1: Navigation Tabs', () => {
    it('renders Fluency, Sessions, and Contributions nav links', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })

      const nav = screen.getByRole('navigation', { name: /main navigation/i })
      expect(within(nav).getByText('Fluency')).toBeInTheDocument()
      expect(within(nav).getByText('Sessions')).toBeInTheDocument()
      expect(within(nav).getByText('Contributions')).toBeInTheDocument()
    })

    it('highlights the active route', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/') })

      const nav = screen.getByRole('navigation', { name: /main navigation/i })
      const fluencyLink = within(nav).getByText('Fluency').closest('a')
      // Active link uses bg-blue-500 text-white
      expect(fluencyLink?.className).toContain('bg-blue-500')
    })

    it('highlights Sessions link when on /sessions route', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/sessions') })

      const nav = screen.getByRole('navigation', { name: /main navigation/i })
      const sessionsLink = within(nav).getByText('Sessions').closest('a')
      expect(sessionsLink?.className).toContain('bg-blue-500')
    })

    it('nav links preserve scope params when project is selected', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })

      const nav = screen.getByRole('navigation', { name: /main navigation/i })
      const sessionsLink = within(nav).getByText('Sessions').closest('a')
      expect(sessionsLink).toHaveAttribute('href', expect.stringContaining('project=project-a'))
    })

    it('nav links preserve both project and branch params', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a&branch=main') })

      const nav = screen.getByRole('navigation', { name: /main navigation/i })
      const contributionsLink = within(nav).getByText('Contributions').closest('a')
      expect(contributionsLink).toHaveAttribute('href', expect.stringContaining('project=project-a'))
      expect(contributionsLink).toHaveAttribute('href', expect.stringContaining('branch=main'))
    })

    it('nav links have no query params when no scope is set', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/') })

      const nav = screen.getByRole('navigation', { name: /main navigation/i })
      const sessionsLink = within(nav).getByText('Sessions').closest('a')
      expect(sessionsLink).toHaveAttribute('href', '/sessions')
    })
  })

  // Zone 2 tests
  describe('Zone 2: Scope Panel', () => {
    it('renders SCOPE section label', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      expect(screen.getByText('Scope')).toBeInTheDocument()
    })

    it('renders project tree with correct role', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      expect(screen.getByRole('tree', { name: /projects/i })).toBeInTheDocument()
    })

    it('renders all projects as tree items', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })

      const tree = screen.getByRole('tree', { name: /projects/i })
      const treeItems = within(tree).getAllByRole('treeitem')
      expect(treeItems.length).toBeGreaterThanOrEqual(2)
    })

    it('displays project display names', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      expect(screen.getByText('Project A')).toBeInTheDocument()
      expect(screen.getByText('Project B')).toBeInTheDocument()
    })

    it('displays session counts', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      expect(screen.getByText('10')).toBeInTheDocument()
      expect(screen.getByText('5')).toBeInTheDocument()
    })

    it('does NOT show clear button when no project is selected', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      expect(screen.queryByRole('button', { name: /clear scope/i })).not.toBeInTheDocument()
    })

    it('shows clear button when a project is selected', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })
      expect(screen.getByRole('button', { name: /clear scope/i })).toBeInTheDocument()
    })

    it('renders view mode toggle buttons (list and tree)', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      expect(screen.getByRole('button', { name: /list view/i })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /tree view/i })).toBeInTheDocument()
    })

    it('list view is active by default', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      const listButton = screen.getByRole('button', { name: /list view/i })
      expect(listButton).toHaveAttribute('aria-pressed', 'true')
    })

    it('selected project gets aria-selected and aria-current', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })

      const tree = screen.getByRole('tree', { name: /projects/i })
      const treeItems = within(tree).getAllByRole('treeitem')
      const selectedItem = treeItems.find(item => item.getAttribute('aria-selected') === 'true')
      expect(selectedItem).toBeTruthy()
      expect(selectedItem).toHaveAttribute('aria-current', 'page')
    })
  })

  // Zone 3 tests
  describe('Zone 3: Quick Jump', () => {
    it('does NOT render when no project is selected', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })
      expect(screen.queryByRole('navigation', { name: /recent sessions/i })).not.toBeInTheDocument()
    })

    it('renders when a project is selected', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })
      expect(screen.getByRole('navigation', { name: /recent sessions/i })).toBeInTheDocument()
    })

    it('shows recent sessions from the hook', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })
      expect(screen.getByText('Fix auth token refresh')).toBeInTheDocument()
      expect(screen.getByText('Add unit tests')).toBeInTheDocument()
    })

    it('shows "Recent" section label', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })
      const quickJump = screen.getByRole('navigation', { name: /recent sessions/i })
      expect(within(quickJump).getByText('Recent')).toBeInTheDocument()
    })

    it('shows "All" link to Sessions page with project scope', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })
      const allLink = screen.getByText('All').closest('a')
      expect(allLink).toHaveAttribute('href', expect.stringContaining('/sessions'))
      expect(allLink).toHaveAttribute('href', expect.stringContaining('project=project-a'))
    })

    it('session links include project scope param', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })
      const quickJump = screen.getByRole('navigation', { name: /recent sessions/i })
      const sessionLinks = within(quickJump).getAllByRole('link')
      // All session links (not the "All" link) should include project param
      const sessionDetailLinks = sessionLinks.filter(link => {
        const href = link.getAttribute('href') || ''
        return href.includes('/sessions/') && href.includes('project=project-a')
      })
      expect(sessionDetailLinks.length).toBeGreaterThanOrEqual(2)
    })

    it('displays relative timestamps for sessions', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })
      const quickJump = screen.getByRole('navigation', { name: /recent sessions/i })
      // First session is ~2h ago, second is ~5h ago
      expect(within(quickJump).getByText('2h')).toBeInTheDocument()
      expect(within(quickJump).getByText('5h')).toBeInTheDocument()
    })
  })

  // Cross-zone interaction tests
  describe('Cross-zone interactions', () => {
    it('all three zones coexist when project is selected', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=project-a') })

      // Zone 1
      expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument()
      // Zone 2
      expect(screen.getByRole('tree', { name: /projects/i })).toBeInTheDocument()
      // Zone 3
      expect(screen.getByRole('navigation', { name: /recent sessions/i })).toBeInTheDocument()
    })

    it('only Zone 1 and Zone 2 render when no project is selected', () => {
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper() })

      // Zone 1
      expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument()
      // Zone 2
      expect(screen.getByRole('tree', { name: /projects/i })).toBeInTheDocument()
      // Zone 3 should NOT be present
      expect(screen.queryByRole('navigation', { name: /recent sessions/i })).not.toBeInTheDocument()
    })
  })

  // Edge cases
  describe('Edge cases', () => {
    it('renders with empty projects array', () => {
      render(<Sidebar projects={[]} />, { wrapper: createWrapper() })

      // Navigation should still render
      expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument()
      // Tree should exist but be empty
      const tree = screen.getByRole('tree', { name: /projects/i })
      expect(within(tree).queryAllByRole('treeitem')).toHaveLength(0)
    })

    it('renders with single project', () => {
      render(<Sidebar projects={[mockProjects[0]]} />, { wrapper: createWrapper() })

      expect(screen.getByText('Project A')).toBeInTheDocument()
      const tree = screen.getByRole('tree', { name: /projects/i })
      expect(within(tree).getAllByRole('treeitem')).toHaveLength(1)
    })

    it('handles unknown project in URL gracefully', () => {
      // URL has project=unknown but it is not in the projects array
      render(<Sidebar projects={mockProjects} />, { wrapper: createWrapper('/?project=unknown') })

      // Should still render without crashing
      expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument()
      // Clear button should still appear (since URL param is set)
      expect(screen.getByRole('button', { name: /clear scope/i })).toBeInTheDocument()
      // Quick jump should render (hook is called with the URL project param)
      expect(screen.getByRole('navigation', { name: /recent sessions/i })).toBeInTheDocument()
    })
  })
})
