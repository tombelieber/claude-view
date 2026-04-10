import { act, renderHook } from '@testing-library/react'
import type { Mock } from 'vitest'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useCardDelegation } from '../useCardDelegation'

/**
 * Helper: advance fake timers enough to flush dispatchKeys inter-key delays
 * without triggering the 2000ms resolved timeout.
 *
 * dispatchKeys uses 200ms between keys. We advance in small steps to
 * let the async promise chain resolve between each tick.
 */
async function flushDispatchKeys(keyCount: number) {
  for (let i = 0; i < keyCount; i++) {
    await vi.advanceTimersByTimeAsync(200)
  }
}

describe('useCardDelegation', () => {
  let sendKeys: (data: string) => void

  beforeEach(() => {
    sendKeys = vi.fn<(data: string) => void>()
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('starts in idle state', () => {
    const { result } = renderHook(() =>
      useCardDelegation({ sendKeys, isConnected: true }),
    )
    expect(result.current.state).toBe('idle')
  })

  it('transitions idle -> sending -> sent -> resolved on delegateSelectOption', async () => {
    const { result } = renderHook(() =>
      useCardDelegation({ sendKeys, isConnected: true }),
    )

    // Start the delegation — index 1 = [DOWN, ENTER] = 2 keys
    act(() => {
      result.current.delegateSelectOption(1)
    })

    // Should be 'sending' immediately
    expect(result.current.state).toBe('sending')

    // Advance through the 2 inter-key delays (200ms each)
    await act(async () => {
      await flushDispatchKeys(2)
    })

    // Should be 'sent' after dispatch completes
    expect(result.current.state).toBe('sent')

    // Advance past the 2s resolved timeout
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000)
    })

    expect(result.current.state).toBe('resolved')
  })

  it('cannot send while already sending', async () => {
    const { result } = renderHook(() =>
      useCardDelegation({ sendKeys, isConnected: true }),
    )

    // Start first delegation (index 0 = [ENTER] = 1 key)
    act(() => {
      result.current.delegateSelectOption(0)
    })
    expect(result.current.state).toBe('sending')

    // Try second delegation while still sending
    ;(sendKeys as Mock).mockClear()
    act(() => {
      result.current.delegateSelectOption(1)
    })

    // sendKeys should NOT have been called again
    expect(sendKeys).not.toHaveBeenCalled()

    // Clean up timers
    await act(async () => {
      await vi.runAllTimersAsync()
    })
  })

  it('cannot send when disconnected', () => {
    const { result } = renderHook(() =>
      useCardDelegation({ sendKeys, isConnected: false }),
    )

    act(() => {
      result.current.delegateSelectOption(0)
    })

    // Should stay idle -- never entered sending
    expect(result.current.state).toBe('idle')
    expect(sendKeys).not.toHaveBeenCalled()
  })

  it('reset returns to idle from any state', async () => {
    const { result } = renderHook(() =>
      useCardDelegation({ sendKeys, isConnected: true }),
    )

    // Drive to 'sent' state — freeText('hello') = ['hello', ENTER] = 2 keys
    act(() => {
      result.current.delegateFreeText('hello')
    })

    await act(async () => {
      await flushDispatchKeys(2)
    })

    expect(result.current.state).toBe('sent')

    // Reset
    act(() => {
      result.current.reset()
    })

    expect(result.current.state).toBe('idle')
  })

  it('delegateMultiSelect sends keys for multiple indices', async () => {
    const { result } = renderHook(() =>
      useCardDelegation({ sendKeys, isConnected: true }),
    )

    // indices [0, 2] = [SPACE, DOWN, DOWN, SPACE, ENTER] = 5 keys
    act(() => {
      result.current.delegateMultiSelect([0, 2])
    })

    expect(result.current.state).toBe('sending')

    await act(async () => {
      await flushDispatchKeys(5)
    })

    expect(result.current.state).toBe('sent')
    expect(sendKeys).toHaveBeenCalled()
  })

  it('delegatePlanApproval sends correct keys', async () => {
    const { result } = renderHook(() =>
      useCardDelegation({ sendKeys, isConnected: true }),
    )

    // approved=true = [ENTER] = 1 key
    act(() => {
      result.current.delegatePlanApproval(true)
    })

    await act(async () => {
      await flushDispatchKeys(1)
    })

    expect(result.current.state).toBe('sent')
    // Approved = Enter only
    expect(sendKeys).toHaveBeenCalledWith('\r')
  })
})
