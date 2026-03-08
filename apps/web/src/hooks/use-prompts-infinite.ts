// src/hooks/use-prompts-infinite.ts
/**
 * Infinite-scroll query hook for the prompt history list.
 *
 * Forked from use-sessions-infinite.ts — same TanStack Query v5 pattern,
 * targeting the GET /api/prompts endpoint.
 */

import { useInfiniteQuery } from '@tanstack/react-query'
import type { PromptListResponse } from '../types/generated/PromptListResponse'

const PAGE_SIZE = 30

export interface PromptsQueryParams {
  search?: string
  project?: string
  intents?: string[]
  branches?: string[]
  models?: string[]
  hasPaste?: 'any' | 'yes' | 'no'
  complexity?: string
  templateMatch?: string
  sort?: string
  timeAfter?: number
  timeBefore?: number
}

function buildSearchParams(params: PromptsQueryParams, offset: number): URLSearchParams {
  const sp = new URLSearchParams()
  sp.set('limit', String(PAGE_SIZE))
  sp.set('offset', String(offset))

  if (params.sort) sp.set('sort', params.sort)
  if (params.search) sp.set('q', params.search)
  if (params.project) sp.set('project', params.project)

  if (params.intents && params.intents.length > 0) sp.set('intent', params.intents.join(','))
  if (params.branches && params.branches.length > 0) sp.set('branches', params.branches.join(','))
  if (params.models && params.models.length > 0) sp.set('models', params.models.join(','))

  if (params.hasPaste === 'yes') sp.set('has_paste', 'true')
  if (params.hasPaste === 'no') sp.set('has_paste', 'false')

  if (params.complexity && params.complexity !== 'any') sp.set('complexity', params.complexity)
  if (params.templateMatch && params.templateMatch !== 'any')
    sp.set('template_match', params.templateMatch)

  if (params.timeAfter) sp.set('time_after', String(params.timeAfter))
  if (params.timeBefore) sp.set('time_before', String(params.timeBefore))

  return sp
}

async function fetchPrompts(
  params: PromptsQueryParams,
  offset: number,
): Promise<PromptListResponse> {
  const sp = buildSearchParams(params, offset)
  const response = await fetch(`/api/prompts?${sp}`)
  if (!response.ok) throw new Error('Failed to fetch prompts')
  return response.json()
}

export function usePromptsInfinite(params: PromptsQueryParams) {
  return useInfiniteQuery({
    queryKey: ['prompts', params],
    queryFn: ({ pageParam }) => fetchPrompts(params, pageParam),
    initialPageParam: 0,
    getNextPageParam: (lastPage, _allPages, lastPageParam) =>
      lastPage.hasMore ? lastPageParam + PAGE_SIZE : undefined,
    select: (data) => ({
      prompts: data.pages.flatMap((p) => p.prompts),
      total: data.pages[0]?.total ?? 0,
    }),
  })
}
