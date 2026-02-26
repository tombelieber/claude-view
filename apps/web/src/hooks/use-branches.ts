// src/hooks/use-branches.ts
import { useQuery } from '@tanstack/react-query';
import type { BranchesResponse } from '../types/generated/BranchesResponse';

/**
 * Fetch distinct branches with session counts for a specific project.
 *
 * @param projectId - The project identifier
 * @returns Promise resolving to BranchesResponse
 */
async function fetchProjectBranches(projectId: string): Promise<BranchesResponse> {
  const response = await fetch(`/api/projects/${encodeURIComponent(projectId)}/branches`);
  if (!response.ok) throw new Error('Failed to fetch project branches');
  return response.json();
}

/**
 * Fetch all distinct branches across all projects.
 *
 * @returns Promise resolving to array of branch name strings
 */
async function fetchAllBranches(): Promise<string[]> {
  const response = await fetch('/api/branches');
  if (!response.ok) throw new Error('Failed to fetch branches');
  return response.json();
}

/**
 * Hook to fetch and cache the list of all branches across all projects.
 *
 * Returns:
 * - data: Array of branch name strings
 * - isLoading: Boolean indicating if the query is in progress
 * - error: Error object if the query failed
 * - refetch: Function to manually refetch the data
 *
 * @example
 * ```tsx
 * const { data: branches = [], isLoading } = useBranches();
 * ```
 */
export function useBranches() {
  return useQuery({
    queryKey: ['branches'],
    queryFn: fetchAllBranches,
    staleTime: 5 * 60 * 1000, // 5 minutes
    gcTime: 10 * 60 * 1000, // 10 minutes
  });
}

/**
 * Hook to fetch and cache the list of branches with session counts for a project.
 *
 * Returns:
 * - data: BranchesResponse with array of {branch, count} objects
 * - isLoading: Boolean indicating if the query is in progress
 * - error: Error object if the query failed
 * - refetch: Function to manually refetch the data
 *
 * @example
 * ```tsx
 * const { data, isLoading, error, refetch } = useProjectBranches('my-project');
 * if (isLoading) return <Skeleton />;
 * if (error) return <ErrorState onRetry={refetch} />;
 * return <BranchList branches={data.branches} />;
 * ```
 */
export function useProjectBranches(projectId: string | undefined) {
  return useQuery({
    queryKey: ['project-branches', projectId],
    queryFn: () => fetchProjectBranches(projectId!),
    enabled: !!projectId,
    // Branches don't change frequently, so we can cache for longer
    staleTime: 5 * 60 * 1000, // 5 minutes
    gcTime: 10 * 60 * 1000, // 10 minutes (formerly cacheTime)
  });
}
