import { colors, fontSize, spacing } from '@claude-view/design-tokens'
import { defaultConfig } from '@tamagui/config/v5'
import { createFont, createTamagui } from 'tamagui'

// Flatten nested color objects: { gray: { 900: '#...' } } → { gray900: '#...' }
// This lets components use $gray900 instead of $gray.900 (which Tamagui v2 doesn't resolve)
function flattenColors(nested: Record<string, Record<string, string>>): Record<string, string> {
  const flat: Record<string, string> = {}
  for (const [group, shades] of Object.entries(nested)) {
    for (const [shade, value] of Object.entries(shades)) {
      flat[`${group}${shade}`] = value
    }
  }
  return flat
}

const flatColors = flattenColors(colors as Record<string, Record<string, string>>)

// Register monospace font for cost/data displays
const monoFont = createFont({
  family: 'monospace',
  size: {
    1: fontSize.xs,
    2: fontSize.sm,
    3: fontSize.base,
    4: fontSize.lg,
    5: fontSize.xl,
    6: fontSize['2xl'],
    7: fontSize['3xl'],
    // Named aliases matching design-tokens
    xs: fontSize.xs,
    sm: fontSize.sm,
    base: fontSize.base,
    lg: fontSize.lg,
    xl: fontSize.xl,
  },
  lineHeight: {
    1: fontSize.xs * 1.5,
    2: fontSize.sm * 1.5,
    3: fontSize.base * 1.5,
    4: fontSize.lg * 1.5,
    5: fontSize.xl * 1.5,
    6: fontSize['2xl'] * 1.5,
    7: fontSize['3xl'] * 1.5,
  },
  weight: {
    4: '400',
    7: '700',
  },
  letterSpacing: {
    4: 0,
  },
})

// Extend default body/heading fonts with named size aliases
const bodyFont = createFont({
  ...defaultConfig.fonts.body,
  size: {
    ...defaultConfig.fonts.body.size,
    xs: fontSize.xs,
    sm: fontSize.sm,
    base: fontSize.base,
    lg: fontSize.lg,
    xl: fontSize.xl,
  },
})

const headingFont = createFont({
  ...defaultConfig.fonts.heading,
  size: {
    ...defaultConfig.fonts.heading.size,
    xs: fontSize.xs,
    sm: fontSize.sm,
    base: fontSize.base,
    lg: fontSize.lg,
    xl: fontSize.xl,
  },
})

const config = createTamagui({
  ...defaultConfig,
  tokens: {
    ...defaultConfig.tokens,
    color: {
      ...defaultConfig.tokens.color,
      ...flatColors,
    },
    space: {
      ...defaultConfig.tokens.space,
      ...spacing,
    },
  },
  fonts: {
    body: bodyFont,
    heading: headingFont,
    mono: monoFont,
  },
})

export default config
export type Conf = typeof config

declare module 'tamagui' {
  interface TamaguiCustomConfig extends Conf {}
}
