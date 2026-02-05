// src/components/FilterPopover.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FilterPopover } from './FilterPopover';
import { DEFAULT_FILTERS } from '../hooks/use-session-filters';

const TEST_BRANCHES = ['main', 'dev', 'feature/auth'];

describe('FilterPopover', () => {
  it('renders trigger button with filter count', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    expect(screen.getByRole('button', { name: /filters/i })).toBeInTheDocument();
  });

  it('shows active filter count badge when filters are active', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();
    const filters = { ...DEFAULT_FILTERS, hasCommits: 'yes' as const };

    render(
      <FilterPopover filters={filters} onChange={onChange} onClear={onClear} activeCount={1} branches={TEST_BRANCHES} />
    );

    expect(screen.getByText('1')).toBeInTheDocument();
  });

  it('opens popover when trigger is clicked', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    expect(screen.getByRole('dialog', { name: /filter sessions/i })).toBeInTheDocument();
  });

  it('shows all filter options when open', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
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

  it('calls onChange immediately when a filter option is clicked', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // Click "Has" under Commits
    const hasButton = screen.getByText('Has');
    fireEvent.click(hasButton);

    expect(onChange).toHaveBeenCalledWith({ ...DEFAULT_FILTERS, hasCommits: 'yes' });
  });

  it('calls onClear when Reset all is clicked', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();
    const filters = { ...DEFAULT_FILTERS, hasCommits: 'yes' as const };

    render(
      <FilterPopover filters={filters} onChange={onChange} onClear={onClear} activeCount={1} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    const resetButton = screen.getByRole('button', { name: /reset all/i });
    fireEvent.click(resetButton);

    expect(onClear).toHaveBeenCalled();
  });

  it('shows branch checkboxes from passed branches prop', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // All branches should be visible as checkbox labels
    expect(screen.getByText('main')).toBeInTheDocument();
    expect(screen.getByText('dev')).toBeInTheDocument();
    expect(screen.getByText('feature/auth')).toBeInTheDocument();
  });

  it('calls onChange when branch checkbox is toggled', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    // Check the "main" branch checkbox
    const mainCheckbox = screen.getByLabelText('main');
    fireEvent.click(mainCheckbox);

    expect(onChange).toHaveBeenCalledWith({ ...DEFAULT_FILTERS, branches: ['main'] });
  });

  it('shows >2h option in duration filter', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    expect(screen.getByText('>30m')).toBeInTheDocument();
    expect(screen.getByText('>1h')).toBeInTheDocument();
    expect(screen.getByText('>2h')).toBeInTheDocument();
  });

  it('shows re-edit rate filter options', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    expect(screen.getByText('Re-edit rate')).toBeInTheDocument();
    expect(screen.getByText('High (>20%)')).toBeInTheDocument();
  });

  it('shows file count filter options', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    expect(screen.getByText('Files edited')).toBeInTheDocument();
    expect(screen.getByText('>5')).toBeInTheDocument();
    expect(screen.getByText('>10')).toBeInTheDocument();
    expect(screen.getByText('>20')).toBeInTheDocument();
  });

  it('shows token range filter options', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    expect(screen.getByText('Token usage')).toBeInTheDocument();
    expect(screen.getByText('>10K')).toBeInTheDocument();
    expect(screen.getByText('>50K')).toBeInTheDocument();
    expect(screen.getByText('>100K')).toBeInTheDocument();
  });

  it('only shows search input when more than 5 branches', () => {
    const onChange = vi.fn();
    const onClear = vi.fn();

    // 3 branches - no search
    render(
      <FilterPopover filters={DEFAULT_FILTERS} onChange={onChange} onClear={onClear} activeCount={0} branches={TEST_BRANCHES} />
    );

    const trigger = screen.getByRole('button', { name: /filters/i });
    fireEvent.click(trigger);

    expect(screen.queryByPlaceholderText(/search branches/i)).not.toBeInTheDocument();
  });
});
