import { render, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'

const state = vi.hoisted(() => ({
  config: {
    posthogKey: 'phc_test',
    anonymousId: 'legacy-uuid',
    telemetry: 'enabled',
    version: '1',
  },
  init: vi.fn(),
  register: vi.fn(),
  optIn: vi.fn(),
}))
vi.mock('@/hooks/use-config', () => ({ useConfig: () => state.config }))
vi.mock('posthog-js', () => ({
  default: { init: state.init, register: state.register, opt_in_capturing: state.optIn },
}))
vi.mock('@posthog/react', () => ({
  PostHogProvider: ({ children }: { children: React.ReactNode }) => children,
}))

import { PostHogTelemetryProvider } from './PostHogTelemetryProvider'

beforeEach(() => {
  vi.clearAllMocks()
  state.config = {
    posthogKey: 'phc_test',
    anonymousId: 'legacy-uuid',
    telemetry: 'enabled',
    version: '1',
  }
})

it('wires the product boundary into the actual SDK initializer', async () => {
  render(<PostHogTelemetryProvider>App</PostHogTelemetryProvider>)
  await waitFor(() => expect(state.init).toHaveBeenCalledOnce())
  const options = state.init.mock.calls[0][1]
  expect(options.bootstrap.distinctID).toBe('claude_view:legacy-uuid')
  expect(options.persistence_name).toBe('claude_view_oss_v1')
  expect(options.person_profiles).toBe('never')
  expect(options.capture_performance).toBe(false)
  expect(
    options.before_send({ uuid: '1', event: 'session_opened', properties: {} }).properties.product,
  ).toBe('claude_view')
  expect(state.optIn).toHaveBeenCalledOnce()
})

it('retains saved opt-out and never initializes source builds without a key', () => {
  state.config.telemetry = 'disabled'
  const { unmount } = render(<PostHogTelemetryProvider>App</PostHogTelemetryProvider>)
  expect(state.optIn).not.toHaveBeenCalled()
  unmount()
  vi.clearAllMocks()
  state.config.posthogKey = ''
  render(<PostHogTelemetryProvider>App</PostHogTelemetryProvider>)
  expect(state.init).not.toHaveBeenCalled()
})
