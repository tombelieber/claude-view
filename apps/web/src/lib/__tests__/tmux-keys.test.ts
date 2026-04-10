import { describe, expect, it, vi } from 'vitest'
import {
  dispatchKeys,
  translateFreeText,
  translateMultiSelect,
  translatePlanApproval,
  translateSelectOption,
} from '../tmux-keys'

// ANSI escape codes — must match the module's constants
const DOWN = '\x1B[B'
const ENTER = '\r'
const SPACE = ' '

// ---------------------------------------------------------------------------
// translateSelectOption
// ---------------------------------------------------------------------------

describe('translateSelectOption', () => {
  it('index 0 → just Enter (first option is pre-selected)', () => {
    expect(translateSelectOption(0)).toEqual([ENTER])
  })

  it('index 1 → Down + Enter', () => {
    expect(translateSelectOption(1)).toEqual([DOWN, ENTER])
  })

  it('index 2 → Down×2 + Enter', () => {
    expect(translateSelectOption(2)).toEqual([DOWN, DOWN, ENTER])
  })

  it('index 4 → Down×4 + Enter', () => {
    expect(translateSelectOption(4)).toEqual([DOWN, DOWN, DOWN, DOWN, ENTER])
  })
})

// ---------------------------------------------------------------------------
// translateMultiSelect
// ---------------------------------------------------------------------------

describe('translateMultiSelect', () => {
  it('single item [0] → Space + Enter', () => {
    expect(translateMultiSelect([0])).toEqual([SPACE, ENTER])
  })

  it('[0, 2] → Space, Down×2, Space, Enter', () => {
    expect(translateMultiSelect([0, 2])).toEqual([SPACE, DOWN, DOWN, SPACE, ENTER])
  })

  it('[1, 3] → Down, Space, Down×2, Space, Enter', () => {
    expect(translateMultiSelect([1, 3])).toEqual([DOWN, SPACE, DOWN, DOWN, SPACE, ENTER])
  })

  it('[3, 1] unsorted → sorts to [1, 3], same result', () => {
    expect(translateMultiSelect([3, 1])).toEqual([DOWN, SPACE, DOWN, DOWN, SPACE, ENTER])
  })

  it('empty [] → just Enter (submit with no selection)', () => {
    expect(translateMultiSelect([])).toEqual([ENTER])
  })
})

// ---------------------------------------------------------------------------
// translateFreeText
// ---------------------------------------------------------------------------

describe('translateFreeText', () => {
  it('"hello" → ["hello", Enter]', () => {
    expect(translateFreeText('hello')).toEqual(['hello', ENTER])
  })

  it('empty string → ["", Enter]', () => {
    expect(translateFreeText('')).toEqual(['', ENTER])
  })

  it('multi-word text preserved as single chunk', () => {
    expect(translateFreeText('yes please')).toEqual(['yes please', ENTER])
  })
})

// ---------------------------------------------------------------------------
// translatePlanApproval
// ---------------------------------------------------------------------------

describe('translatePlanApproval', () => {
  it('approved=true → Enter (default selection)', () => {
    expect(translatePlanApproval(true)).toEqual([ENTER])
  })

  it('approved=false → Down + Enter', () => {
    expect(translatePlanApproval(false)).toEqual([DOWN, ENTER])
  })
})

// ---------------------------------------------------------------------------
// dispatchKeys
// ---------------------------------------------------------------------------

describe('dispatchKeys', () => {
  it('calls sendFn for each key in order', async () => {
    const calls: string[] = []
    const sendFn = (data: string) => calls.push(data)

    await dispatchKeys(sendFn, [DOWN, DOWN, ENTER], 0)

    expect(calls).toEqual([DOWN, DOWN, ENTER])
  })

  it('respects delay between keys', async () => {
    const timestamps: number[] = []
    const sendFn = () => timestamps.push(Date.now())

    await dispatchKeys(sendFn, ['a', 'b', 'c'], 50)

    // Each gap should be >= 50ms (allow 40ms tolerance for timer jitter)
    expect(timestamps).toHaveLength(3)
    expect(timestamps[1]! - timestamps[0]!).toBeGreaterThanOrEqual(40)
    expect(timestamps[2]! - timestamps[1]!).toBeGreaterThanOrEqual(40)
  })

  it('works with delayMs=0 (no delay)', async () => {
    const calls: string[] = []
    const sendFn = (data: string) => calls.push(data)

    await dispatchKeys(sendFn, ['x', 'y'], 0)

    expect(calls).toEqual(['x', 'y'])
  })

  it('handles empty keys array', async () => {
    const sendFn = vi.fn()

    await dispatchKeys(sendFn, [], 200)

    expect(sendFn).not.toHaveBeenCalled()
  })
})
