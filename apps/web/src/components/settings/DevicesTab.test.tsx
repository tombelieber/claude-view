import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

// ── Supabase mock ─────────────────────────────────────────────────────────
// Keep mutable handles so each test can control select()/functions.invoke().
// Use vi.hoisted to create spies that the mock factory can reference.
const mocks = vi.hoisted(() => {
  type AnyResolved = { data: unknown; error: unknown }
  return {
    selectSpy: vi.fn(),
    orderSpy: vi.fn<() => Promise<AnyResolved>>(() => Promise.resolve({ data: [], error: null })),
    invokeSpy: vi.fn<(...args: unknown[]) => Promise<AnyResolved>>(() =>
      Promise.resolve({ data: null, error: null }),
    ),
    removeChannelSpy: vi.fn(),
    channelSpy: vi.fn(),
    getSessionSpy: vi.fn<() => Promise<{ data: { session: unknown } }>>(() =>
      Promise.resolve({
        data: {
          session: {
            user: { id: 'user-123', email: 'test@example.com', user_metadata: {} },
            access_token: 'fake-jwt',
          },
        },
      }),
    ),
    authCallbackRef: { current: null as ((event: string, session: unknown) => void) | null },
  }
})

vi.mock('../../lib/supabase', () => ({
  supabase: {
    auth: {
      onAuthStateChange: (cb: (event: string, session: unknown) => void) => {
        mocks.authCallbackRef.current = cb
        return { data: { subscription: { unsubscribe: vi.fn() } } }
      },
      getSession: mocks.getSessionSpy,
      signOut: vi.fn(),
    },
    from: vi.fn(() => ({
      select: mocks.selectSpy.mockReturnValue({ order: mocks.orderSpy }),
    })),
    functions: { invoke: mocks.invokeSpy },
    channel: mocks.channelSpy.mockReturnValue({
      on: vi.fn().mockReturnThis(),
      subscribe: vi.fn().mockReturnValue({ unsubscribe: vi.fn() }),
    }),
    removeChannel: mocks.removeChannelSpy,
  },
  getAccessToken: vi.fn().mockResolvedValue('fake-jwt'),
}))

vi.mock('@posthog/react', () => ({
  usePostHog: () => ({ identify: vi.fn() }),
  PostHogProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}))

// sonner toast spy
const toastMocks = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}))
vi.mock('sonner', () => ({
  toast: {
    success: (...args: unknown[]) => toastMocks.toastSuccess(...args),
    error: (...args: unknown[]) => toastMocks.toastError(...args),
  },
}))

// qrcode.react renders <svg>; happy-dom is fine with it, but make it cheap
vi.mock('qrcode.react', () => ({
  QRCodeSVG: ({ value }: { value: string }) => (
    <svg data-testid="qr-code" aria-label={value}>
      <title>{value}</title>
    </svg>
  ),
}))

import { AuthProvider } from '../../hooks/use-auth'
import { DevicesTab } from './DevicesTab'

function wrap(node: ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return (
    <QueryClientProvider client={client}>
      <AuthProvider>{node}</AuthProvider>
    </QueryClientProvider>
  )
}

async function signIn() {
  await act(async () => {
    mocks.authCallbackRef.current?.('SIGNED_IN', {
      user: { id: 'user-123', email: 'test@example.com', user_metadata: {} },
    })
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  mocks.authCallbackRef.current = null
  mocks.orderSpy.mockImplementation(() => Promise.resolve({ data: [], error: null }))
  mocks.invokeSpy.mockImplementation(() => Promise.resolve({ data: null, error: null }))
  mocks.getSessionSpy.mockImplementation(() =>
    Promise.resolve({
      data: {
        session: {
          user: { id: 'user-123', email: 'test@example.com', user_metadata: {} },
          access_token: 'fake-jwt',
        },
      },
    }),
  )
})

describe('DevicesTab', () => {
  it('shows a sign-in nudge when no user is signed in', async () => {
    mocks.getSessionSpy.mockResolvedValueOnce({ data: { session: null } })
    render(wrap(<DevicesTab />))
    await waitFor(() => {
      expect(screen.getByText(/sign in to pair your phone/i)).toBeInTheDocument()
    })
  })

  it('renders "No devices paired yet" when the list is empty', async () => {
    mocks.orderSpy.mockResolvedValueOnce({ data: [], error: null })
    render(wrap(<DevicesTab />))
    await signIn()
    await waitFor(() => {
      expect(screen.getByText(/no devices paired yet/i)).toBeInTheDocument()
    })
    expect(screen.getByRole('button', { name: /pair a new device/i })).toBeEnabled()
  })

  it('renders a paired device row and a Revoke button', async () => {
    mocks.orderSpy.mockResolvedValueOnce({
      data: [
        {
          device_id: 'ios-0123456789abcdef',
          user_id: 'user-123',
          platform: 'ios',
          display_name: 'iPhone 15',
          last_seen_at: new Date().toISOString(),
          revoked_at: null,
          revoked_reason: null,
          created_at: new Date().toISOString(),
          ed25519_pubkey: 'k1',
          x25519_pubkey: 'k2',
          app_version: '0.38.0',
          os_version: 'iOS 19.1',
          last_ip: null,
          last_user_agent: null,
        },
      ],
      error: null,
    })
    render(wrap(<DevicesTab />))
    await signIn()
    await waitFor(() => {
      expect(screen.getByText('iPhone 15')).toBeInTheDocument()
    })
    expect(screen.getByRole('button', { name: /revoke iphone 15/i })).toBeInTheDocument()
  })

  it('surfaces a clear error message when device fetch fails', async () => {
    mocks.orderSpy.mockResolvedValueOnce({
      data: null,
      error: { message: 'Supabase unreachable' },
    })
    render(wrap(<DevicesTab />))
    await signIn()
    await waitFor(() => {
      expect(screen.getByText(/couldn't load devices/i)).toBeInTheDocument()
      expect(screen.getByText(/supabase unreachable/i)).toBeInTheDocument()
    })
  })

  it('opens the pairing dialog and transitions to showing-qr state', async () => {
    mocks.orderSpy.mockResolvedValue({ data: [], error: null })
    const expiresAt = new Date(Date.now() + 5 * 60 * 1000).toISOString()
    mocks.invokeSpy.mockResolvedValueOnce({
      data: {
        token: 'test-token-xyz',
        relay_ws_url: 'wss://relay.example/ws',
        expires_at: expiresAt,
      },
      error: null,
    })

    render(wrap(<DevicesTab />))
    await signIn()
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /pair a new device/i })).toBeInTheDocument()
    })

    fireEvent.click(screen.getByRole('button', { name: /pair a new device/i }))

    await waitFor(() => {
      // QR code mock renders with aria-label = pairing URL
      expect(screen.getByLabelText('claude-view-pair://test-token-xyz')).toBeInTheDocument()
    })
    expect(screen.getByText(/expires in/i)).toBeInTheDocument()
  })

  it('shows "Try again" when pair-offer returns an error', async () => {
    mocks.orderSpy.mockResolvedValue({ data: [], error: null })
    mocks.invokeSpy.mockResolvedValueOnce({
      data: null,
      error: { message: 'Supabase edge function failed' },
    })
    render(wrap(<DevicesTab />))
    await signIn()
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /pair a new device/i })).toBeInTheDocument()
    })
    fireEvent.click(screen.getByRole('button', { name: /pair a new device/i }))
    await waitFor(() => {
      expect(screen.getByText(/supabase edge function failed/i)).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument()
    })
  })

  it('revoke flow: clicking Revoke opens confirm, confirming invokes edge function', async () => {
    mocks.orderSpy.mockResolvedValue({
      data: [
        {
          device_id: 'ios-0123456789abcdef',
          user_id: 'user-123',
          platform: 'ios',
          display_name: 'iPhone 15',
          last_seen_at: new Date().toISOString(),
          revoked_at: null,
          revoked_reason: null,
          created_at: new Date().toISOString(),
          ed25519_pubkey: 'k1',
          x25519_pubkey: 'k2',
          app_version: null,
          os_version: null,
          last_ip: null,
          last_user_agent: null,
        },
      ],
      error: null,
    })
    mocks.invokeSpy.mockResolvedValueOnce({
      data: { device: { device_id: 'ios-0123456789abcdef', revoked_at: new Date().toISOString() } },
      error: null,
    })

    render(wrap(<DevicesTab />))
    await signIn()
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /revoke iphone 15/i })).toBeInTheDocument()
    })

    fireEvent.click(screen.getByRole('button', { name: /revoke iphone 15/i }))

    const confirmBtn = await screen.findByRole('button', { name: /^revoke$/i })
    fireEvent.click(confirmBtn)

    await waitFor(() => {
      expect(mocks.invokeSpy).toHaveBeenCalledWith(
        'devices-revoke',
        expect.objectContaining({
          body: { device_id: 'ios-0123456789abcdef', reason: 'user_action' },
        }),
      )
    })
    await waitFor(() => {
      expect(toastMocks.toastSuccess).toHaveBeenCalled()
    })
  })
})
