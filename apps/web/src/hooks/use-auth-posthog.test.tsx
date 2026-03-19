import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mockIdentify = vi.fn()
vi.mock('@posthog/react', () => ({
  usePostHog: () => ({ identify: mockIdentify }),
  PostHogProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}))

let authCallback: ((event: string, session: any) => void) | null = null
vi.mock('../lib/supabase', () => ({
  supabase: {
    auth: {
      onAuthStateChange: (cb: (event: string, session: any) => void) => {
        authCallback = cb
        return { data: { subscription: { unsubscribe: vi.fn() } } }
      },
      getSession: vi.fn().mockResolvedValue({ data: { session: null } }),
    },
  },
}))

describe('Auth + PostHog identify', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    authCallback = null
  })

  it('calls posthog.identify with user ID and email on SIGNED_IN', async () => {
    const { AuthProvider } = await import('./use-auth')
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    render(
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <div>test</div>
        </AuthProvider>
      </QueryClientProvider>,
    )
    authCallback?.('SIGNED_IN', {
      user: { id: 'user-123', email: 'test@example.com', user_metadata: {} },
    })
    await waitFor(() => {
      expect(mockIdentify).toHaveBeenCalledWith('user-123', { email: 'test@example.com' })
    })
  })
})
