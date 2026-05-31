import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { type ReactNode, createElement } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { useClaudeHomeEntries, useWorkflowRuns, useWorkflows } from './use-workflows'

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(QueryClientProvider, { client: queryClient }, children)
  }
}

describe('useWorkflows', () => {
  it('returns empty array when no legacy definitions exist', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve([]) }))
    const { result } = renderHook(() => useWorkflows(), { wrapper: createWrapper() })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([])
  })

  it('returns workflow run response from Claude artifacts endpoint', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            runs: [
              {
                sessionId: 'sess-1',
                runId: 'wf_123',
                projectDir: 'proj',
                workflowName: 'Dynamic run',
                status: 'completed',
                summary: 'Done',
                defaultModel: 'claude',
                startTime: 1,
                durationMs: 2,
                totalTokens: 3,
                totalToolCalls: 4,
                agentCount: 1,
                phaseCount: 1,
                updatedAt: 5,
                scriptPreview: null,
                resultPreview: null,
                hasSummaryJson: true,
                hasJournal: true,
              },
            ],
          }),
      }),
    )
    const { result } = renderHook(() => useWorkflowRuns(), { wrapper: createWrapper() })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.runs[0].workflowName).toBe('Dynamic run')
  })

  it('returns Claude home entries', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve([
            {
              kind: 'session-env',
              name: 'session-env',
              relativePath: 'session-env',
              path: '/tmp/session-env',
              isDirectory: true,
              itemCount: 1,
              sizeBytes: 0,
              modifiedAt: 1,
              preview: null,
              previewTruncated: false,
              metadataOnly: true,
            },
          ]),
      }),
    )
    const { result } = renderHook(() => useClaudeHomeEntries(), { wrapper: createWrapper() })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.[0].metadataOnly).toBe(true)
  })
})
