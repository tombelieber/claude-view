import { describe, it, expect } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useFacetIngest } from './use-facet-ingest'

describe('useFacetIngest', () => {
  it('starts with null progress', () => {
    const { result } = renderHook(() => useFacetIngest())
    expect(result.current.progress).toBeNull()
    expect(result.current.isRunning).toBe(false)
  })

  it('has trigger function', () => {
    const { result } = renderHook(() => useFacetIngest())
    expect(typeof result.current.trigger).toBe('function')
  })
})
