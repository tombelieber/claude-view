// src/components/FilterPopover.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FilterPopover } from './FilterPopover';
import { DEFAULT_FILTERS } from '../hooks/use-session-filters';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Mock the useBranches hook
vi.mock('../hooks/use-branches', () => ({
  useBranches: () => ({
    data: ['main', 'dev', 'feature/auth'],
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

describe('FilterPopover', () => {
  it('renders trigger button with filter count', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    expect(screen.getByRole('button', { name: /filters/i })).toBeInTheDocument();
  });

  it('shows active filter count badge when filters are active', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();
    const filters = { ...DEFAULT_FILTERS, hasCommits: 'yes' as const };

    renderWithQueryClient(
      <FilterPopover filters={filters} onChange={onChange} onClear={onClear} activeCount={1} />
    );

    expect(screen.getByText('1')).toBeInTheDocument();
  });

  it('opens popover when trigger is clicked', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    expect(screen.getByRole('dialog', { name: /filter sessions/i })).toBeInTheDocument();
  });

  it('shows all filter options when open', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // Check for filter section labels
    expect(screen.getByText('Commits')).toBeInTheDocument();
    expect(screen.getByText('Duration')).toBeInTheDocument();
    expect(screen.getByText('Branch')).toBeInTheDocument();
    expect(screen.getByText('Model')).toBeInTheDocument();
    expect(screen.getByText('Has skills')).toBeInTheDocument();
    expect(screen.getByText('Re-edit rate')).toBeInTheDocument();
    expect(screen.getByText('Files edited')).toBeInTheDocument();
    expect(screen.getByText('Token usage')).toBeInTheDocument();
  });

  it('calls onChange when Apply is clicked', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    const applyButton = screen.getByRole('button', { name: /apply/i });
    fireEvent.click(applyButton);

    expect(onChange).toHaveBeenCalledWith(DEFAULT_FILTERS);
  });

  it('calls onClear when Clear is clicked', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();
    const filters = { ...DEFAULT_FILTERS, hasCommits: 'yes' as const };

    renderWithQueryClient(
      <FilterPopover filters={filters} onChange={onChange} onClear={onClear} activeCount={1} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    const clearButton = screen.getByRole('button', { name: /clear/i });
    fireEvent.click(clearButton);

    expect(onClear).toHaveBeenCalled();
  });

  it('allows searching branches', async () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    const searchInput = screen.getByPlaceholderText(/search branches/i);
    fireEvent.change(searchInput, { target: { value: 'feature' } });

    // Wait for debounce (150ms)
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Should show only matching branch
    expect(screen.getByText('feature/auth')).toBeInTheDocument();
    expect(screen.queryByText('main')).not.toBeInTheDocument();
  });

  it('shows >2h option in duration filter', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // Check for all duration options including >2h
    expect(screen.getByText('>30m')).toBeInTheDocument();
    expect(screen.getByText('>1h')).toBeInTheDocument();
    expect(screen.getByText('>2h')).toBeInTheDocument();
  });

  it('shows re-edit rate filter options', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // Check for re-edit rate options
    expect(screen.getByText('Re-edit rate')).toBeInTheDocument();
    expect(screen.getByText('High (>20%)')).toBeInTheDocument();
  });

  it('shows file count filter options', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // Check for file count options
    expect(screen.getByText('Files edited')).toBeInTheDocument();
    expect(screen.getByText('>5')).toBeInTheDocument();
    expect(screen.getByText('>10')).toBeInTheDocument();
    expect(screen.getByText('>20')).toBeInTheDocument();
  });

  it('shows token range filter options', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    renderWithQueryClient(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // Check for token range options
    expect(screen.getByText('Token usage')).toBeInTheDocument();
    expect(screen.getByText('>10K')).toBeInTheDocument();
    expect(screen.getByText('>50K')).toBeInTheDocument();
    expect(screen.getByText('>100K')).toBeInTheDocument();
  });
});
