// src/components/SessionToolbar.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SessionToolbar } from './SessionToolbar';
import { DEFAULT_FILTERS } from '../hooks/use-session-filters';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Mock the useBranches hook
vi.mock('../hooks/use-branches', () => ({
  useBranches: () => ({
    data: ['main', 'dev'],
    isLoading: false,
  }),
}));

function renderWithQueryClient(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

describe('SessionToolbar', () => {
  it('renders all toolbar controls', () => {
    const onFiltersChange = vi.fn();
    const onClearFilters = vi.fn();

    renderWithQueryClient(
      <SessionToolbar
        filters={DEFAULT_FILTERS}
        onFiltersChange={onFiltersChange}
        onClearFilters={onClearFilters}
      />
    );

    // Check for all three main controls
    expect(screen.getByRole('button', { name: /group by/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /filters/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sort/i })).toBeInTheDocument();
  });

  it('shows default group by option (None)', () => {
    const onFiltersChange = vi.fn();
    const onClearFilters = vi.fn();

    renderWithQueryClient(
      <SessionToolbar
        filters={DEFAULT_FILTERS}
        onFiltersChange={onFiltersChange}
        onClearFilters={onClearFilters}
      />
    );

    const groupByButton = screen.getByRole('button', { name: /group by: none/i });
    expect(groupByButton).toBeInTheDocument();
  });

  it('shows default sort option (Most recent)', () => {
    const onFiltersChange = vi.fn();
    const onClearFilters = vi.fn();

    renderWithQueryClient(
      <SessionToolbar
        filters={DEFAULT_FILTERS}
        onFiltersChange={onFiltersChange}
        onClearFilters={onClearFilters}
      />
    );

    const sortButton = screen.getByRole('button', { name: /sort: most recent/i });
    expect(sortButton).toBeInTheDocument();
  });

  it('changes group by when option is selected', () => {
    const onFiltersChange = vi.fn();
    const onClearFilters = vi.fn();

    renderWithQueryClient(
      <SessionToolbar
        filters={DEFAULT_FILTERS}
        onFiltersChange={onFiltersChange}
        onClearFilters={onClearFilters}
      />
    );

    // Open group by dropdown
    const groupByButton = screen.getByRole('button', { name: /group by/i });
    fireEvent.click(groupByButton);

    // Select "Branch" option
    const branchOption = screen.getByRole('option', { name: /branch group by git branch/i });
    fireEvent.click(branchOption);

    expect(onFiltersChange).toHaveBeenCalledWith({
      ...DEFAULT_FILTERS,
      groupBy: 'branch',
    });
  });

  it('changes sort when option is selected', () => {
    const onFiltersChange = vi.fn();
    const onClearFilters = vi.fn();

    renderWithQueryClient(
      <SessionToolbar
        filters={DEFAULT_FILTERS}
        onFiltersChange={onFiltersChange}
        onClearFilters={onClearFilters}
      />
    );

    // Open sort dropdown
    const sortButton = screen.getByRole('button', { name: /sort/i });
    fireEvent.click(sortButton);

    // Select "Most tokens" option
    const tokensOption = screen.getByRole('option', { name: /most tokens/i });
    fireEvent.click(tokensOption);

    expect(onFiltersChange).toHaveBeenCalledWith({
      ...DEFAULT_FILTERS,
      sort: 'tokens',
    });
  });

  it('highlights active controls', () => {
    const onFiltersChange = vi.fn();
    const onClearFilters = vi.fn();
    const activeFilters = {
      ...DEFAULT_FILTERS,
      groupBy: 'branch' as const,
      sort: 'tokens' as const,
      hasCommits: 'yes' as const,
    };

    renderWithQueryClient(
      <SessionToolbar
        filters={activeFilters}
        onFiltersChange={onFiltersChange}
        onClearFilters={onClearFilters}
      />
    );

    const groupByButton = screen.getByRole('button', { name: /group by/i });
    const sortButton = screen.getByRole('button', { name: /sort/i });
    const filterButton = screen.getByRole('button', { name: /filters/i });

    // Check that active controls have the active styling class
    expect(groupByButton.className).toContain('bg-blue');
    expect(sortButton.className).toContain('bg-blue');
    expect(filterButton.className).toContain('bg-blue');
  });

  it('shows filter count badge', () => {
    const onFiltersChange = vi.fn();
    const onClearFilters = vi.fn();
    const activeFilters = {
      ...DEFAULT_FILTERS,
      hasCommits: 'yes' as const,
      branches: ['main'],
    };

    renderWithQueryClient(
      <SessionToolbar
        filters={activeFilters}
        onFiltersChange={onFiltersChange}
        onClearFilters={onClearFilters}
      />
    );

    // Filter count badge should show "2"
    expect(screen.getByText('2')).toBeInTheDocument();
  });
});
