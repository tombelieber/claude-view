/** Design token: typography
 *
 * Font system: Geist Sans (UI) + Geist Mono (code/metrics)
 * Type scale: Apple HIG-inspired with optical letter-spacing
 */
export const fontFamily = {
  sans: [
    'Geist',
    '-apple-system',
    'BlinkMacSystemFont',
    'Segoe UI',
    'Roboto',
    'Helvetica Neue',
    'Arial',
    'sans-serif',
  ],
  mono: [
    'Geist Mono',
    'ui-monospace',
    'SFMono-Regular',
    'SF Mono',
    'Menlo',
    'Consolas',
    'monospace',
  ],
} as const

/**
 * Apple HIG-inspired type scale.
 * Each level has a specific size, line-height, and letter-spacing
 * to create clear visual hierarchy without relying on color alone.
 *
 * - Display/titles: tighter letter-spacing (text "snaps" together)
 * - Body: neutral spacing (comfortable reading)
 * - Captions: slightly open spacing (legibility at small sizes)
 */
export const typeScale = {
  display: { size: 32, lineHeight: 1.1, letterSpacing: -0.03, weight: 700 },
  'title-1': { size: 24, lineHeight: 1.2, letterSpacing: -0.02, weight: 600 },
  'title-2': { size: 20, lineHeight: 1.25, letterSpacing: -0.015, weight: 600 },
  'title-3': { size: 17, lineHeight: 1.3, letterSpacing: -0.01, weight: 600 },
  body: { size: 15, lineHeight: 1.5, letterSpacing: 0, weight: 400 },
  'body-em': { size: 15, lineHeight: 1.5, letterSpacing: 0, weight: 500 },
  callout: { size: 14, lineHeight: 1.45, letterSpacing: 0, weight: 400 },
  'caption-1': { size: 13, lineHeight: 1.4, letterSpacing: 0.01, weight: 400 },
  'caption-2': { size: 11, lineHeight: 1.35, letterSpacing: 0.02, weight: 500 },
} as const

/** Flat size map — aligned with Tailwind @theme overrides */
export const fontSize = {
  xs: 13,
  sm: 14,
  base: 15,
  lg: 17,
  xl: 20,
  '2xl': 24,
  '3xl': 32,
} as const

/** Recharts / chart library font sizes — single source of truth */
export const chartFontSize = {
  axisLabel: 12,
  axisTick: 11,
  tooltip: 13,
  legend: 12,
  annotation: 11,
} as const
