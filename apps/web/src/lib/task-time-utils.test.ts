import { describe, expect, it } from 'vitest'
import { getDisplayLongestTaskSeconds, getDisplayTaskTimeSeconds } from './task-time-utils'

describe('task-time-utils', () => {
  it('prefers totalTaskTimeSeconds when no outlier is detected', () => {
    const result = getDisplayTaskTimeSeconds({
      totalTaskTimeSeconds: 7_200,
      turnDurationAvgMs: 300_000,
      turnCount: 20,
      durationSeconds: 10_000,
    })

    expect(result).toBe(7_200)
  })

  it('falls back to turn-duration estimate when total task time is an outlier', () => {
    const result = getDisplayTaskTimeSeconds({
      totalTaskTimeSeconds: 180_000,
      turnDurationAvgMs: 500_000, // ~8.3m
      turnCount: 20, // ~10,000s estimate
      durationSeconds: 200_000,
    })

    expect(result).toBe(10_000)
  })

  it('falls back to turn-duration max when longest task is an outlier', () => {
    const result = getDisplayLongestTaskSeconds({
      longestTaskSeconds: 120_000,
      turnDurationMaxMs: 900_000, // 15m
      durationSeconds: 200_000,
    })

    expect(result).toBe(900)
  })

  it('falls back to duration when task metrics are missing', () => {
    const result = getDisplayTaskTimeSeconds({
      durationSeconds: 1_234,
    })

    expect(result).toBe(1_234)
  })
})
