import type { ConnectionState } from '@claude-view/shared'
import { Circle, type GetProps, Text, XStack } from 'tamagui'

type CircleBackground = GetProps<typeof Circle>['bg']

// Audit gap #20: Use Tamagui tokens instead of hardcoded hex.
const STATE_CONFIG: Record<ConnectionState, { color: CircleBackground; label: string }> = {
  connected: { color: '$statusActive', label: 'Connected' },
  connecting: { color: '$statusWarning', label: 'Connecting' },
  disconnected: { color: '$statusError', label: 'Mac offline' },
  crypto_error: { color: '$statusError', label: 'Re-pair needed' },
}

export function ConnectionDot({ state }: { state: ConnectionState }) {
  const { color, label } = STATE_CONFIG[state]
  return (
    <XStack items="center" gap="$2">
      <Circle size={8} bg={color} />
      <Text color="$gray400" fontSize="$sm">
        {label}
      </Text>
    </XStack>
  )
}
