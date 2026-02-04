// src/hooks/use-branches.ts
import { useQuery } from '@tanstack/react-query';

/**
 * Fetch distinct list of branch names from the backend.
 *
 * @returns Promise resolving to array of unique branch names, sorted alphabetically
 */
async function fetchBranches(): Promise<string[]> {
  const response = await fetch('/api/branches');
  if (!response.ok) throw new Error('Failed to fetch branches');
  return response.json();
}

/**
 * Hook to fetch and cache the list of all unique branch names across sessions.
 *
 * Returns:
 * - data: Array of branch names (sorted alphabetically)
 * - isLoading: Boolean indicating if the query is in progress
 * - error: Error object if the query failed
 *
 * @example
 * ```tsx
 * const { data: branches, isLoading } = useBranches();
 * if (isLoading) return <Spinner />;
 * return <BranchFilter branches={branches} />;
 * ```
 */
export function useBranches() {
  return useQuery({
    queryKey: ['branches'],
    queryFn: fetchBranches,
    // Branches don't change frequently, so we can cache for longer
    staleTime: 5 * 60 * 1000, // 5 minutes
    gcTime: 10 * 60 * 1000, // 10 minutes (formerly cacheTime)
  });
}
