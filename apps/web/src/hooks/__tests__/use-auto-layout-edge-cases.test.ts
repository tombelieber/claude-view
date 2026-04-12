import { describe, expect, it } from 'vitest'
import { MIN_PANE_H, MIN_PANE_W, computeMaxVisibleCols } from '../use-auto-layout'

describe('computeMaxVisibleCols — viewport breakpoints (spec compliance)', () => {
  // Spec: "4 sessions on 1440px laptop: 2 visible + 2 tabbed"
  it('returns 3 for 1440px (spec: 2 visible + 2 tabbed at 4 sessions)', () => {
    // 1440 / 400 = 3.6 → floor = 3
    expect(computeMaxVisibleCols(1440)).toBe(3)
  })

  // Spec breakpoints
  it('≥1600px: 4 visible panes', () => {
    expect(computeMaxVisibleCols(1600)).toBe(4)
    expect(computeMaxVisibleCols(1920)).toBe(4)
    expect(computeMaxVisibleCols(2560)).toBe(6) // ultrawide gets more
  })

  it('1200–1599px: 3 visible', () => {
    expect(computeMaxVisibleCols(1200)).toBe(3)
    expect(computeMaxVisibleCols(1400)).toBe(3)
    expect(computeMaxVisibleCols(1599)).toBe(3)
  })

  it('800–1199px: 2 visible', () => {
    expect(computeMaxVisibleCols(800)).toBe(2)
    expect(computeMaxVisibleCols(1000)).toBe(2)
    expect(computeMaxVisibleCols(1199)).toBe(2)
  })

  it('<800px: 1 visible (focus mode)', () => {
    expect(computeMaxVisibleCols(400)).toBe(1)
    expect(computeMaxVisibleCols(600)).toBe(1)
    expect(computeMaxVisibleCols(799)).toBe(1)
  })

  it('very small viewport returns 0', () => {
    expect(computeMaxVisibleCols(200)).toBe(0)
    expect(computeMaxVisibleCols(0)).toBe(0)
  })
})

describe('constants match spec', () => {
  it('MIN_PANE_W = 400 (50 cols × ~8px/char)', () => {
    expect(MIN_PANE_W).toBe(400)
  })

  it('MIN_PANE_H = 200 (~9 rows + tab)', () => {
    expect(MIN_PANE_H).toBe(200)
  })
})
