import { colors, spacing } from '@claude-view/design-tokens'
import { defaultConfig } from '@tamagui/config/v5'
import { createTamagui } from 'tamagui'

const config = createTamagui({
  ...defaultConfig,
  tokens: {
    ...defaultConfig.tokens,
    color: {
      ...defaultConfig.tokens.color,
      ...colors,
    },
    space: {
      ...defaultConfig.tokens.space,
      ...spacing,
    },
  },
})

export default config
export type Conf = typeof config

declare module 'tamagui' {
  interface TamaguiCustomConfig extends Conf {}
}
