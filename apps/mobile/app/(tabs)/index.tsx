import { Redirect } from 'expo-router'
import { H1, Spinner, Text, YStack } from 'tamagui'
import { usePairingStatus } from '../../hooks/use-pairing-status'

export default function SessionsScreen() {
  const { isPaired } = usePairingStatus()

  // Loading state
  if (isPaired === null) {
    return (
      <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$background">
        <Spinner size="large" />
      </YStack>
    )
  }

  // Not paired — redirect to pair screen
  if (!isPaired) {
    return <Redirect href="/pair" />
  }

  // Paired — show dashboard (next task)
  return (
    <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$background">
      <H1>Claude View Mobile</H1>
      <Text color="$colorSubtle" marginTop="$2">
        Session monitoring coming soon
      </Text>
    </YStack>
  )
}
