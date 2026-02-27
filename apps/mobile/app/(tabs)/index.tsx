import { groupByStatus } from '@claude-view/shared'
import { Redirect } from 'expo-router'
import { useMemo, useState } from 'react'
import { ScrollView } from 'react-native'
import { SafeAreaView } from 'react-native-safe-area-context'
import { H4, Spinner, Text, XStack, YStack } from 'tamagui'
import { ConnectionDot } from '../../components/ConnectionDot'
import { SessionCard } from '../../components/SessionCard'
import { SummaryBar } from '../../components/SummaryBar'
import { usePairingStatus } from '../../hooks/use-pairing-status'
import { useRelaySessions } from '../../hooks/use-relay-sessions'

export default function SessionsScreen() {
  const { isPaired } = usePairingStatus()
  const { sessions, connectionState } = useRelaySessions()
  const [selectedId, setSelectedId] = useState<string | null>(null)

  // ALL hooks must be called before any conditional returns (Rules of Hooks)
  const sessionList = useMemo(() => Object.values(sessions), [sessions])
  const { needsYou, autonomous } = useMemo(() => groupByStatus(sessionList), [sessionList])

  if (isPaired === null) {
    return (
      <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$gray900">
        <Spinner size="large" />
      </YStack>
    )
  }

  if (!isPaired) {
    return <Redirect href="/pair" />
  }

  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: '#111827' }} edges={['top']}>
      {/* Header */}
      <XStack
        justifyContent="space-between"
        alignItems="center"
        paddingHorizontal="$4"
        paddingVertical="$3"
      >
        <Text color="$gray50" fontWeight="bold" fontSize="$xl">
          Claude View
        </Text>
        <ConnectionDot state={connectionState} />
      </XStack>

      {/* Session list */}
      <ScrollView style={{ flex: 1, paddingHorizontal: 16 }}>
        {needsYou.length > 0 && (
          <YStack marginBottom="$4">
            <H4
              color="$statusWarning"
              fontSize="$xs"
              textTransform="uppercase"
              letterSpacing={1}
              marginBottom="$2"
            >
              Needs You
            </H4>
            {needsYou.map((s) => (
              <SessionCard key={s.id} session={s} onPress={() => setSelectedId(s.id)} />
            ))}
          </YStack>
        )}

        {autonomous.length > 0 && (
          <YStack marginBottom="$4">
            <H4
              color="$statusActive"
              fontSize="$xs"
              textTransform="uppercase"
              letterSpacing={1}
              marginBottom="$2"
            >
              Autonomous
            </H4>
            {autonomous.map((s) => (
              <SessionCard key={s.id} session={s} onPress={() => setSelectedId(s.id)} />
            ))}
          </YStack>
        )}

        {sessionList.length === 0 && (
          <YStack flex={1} alignItems="center" justifyContent="center" paddingVertical="$16">
            <Text color="$gray400" fontSize="$lg">
              {connectionState === 'disconnected' ? 'Mac offline' : 'No active sessions'}
            </Text>
          </YStack>
        )}
      </ScrollView>

      {/* Summary bar */}
      <SummaryBar sessions={sessionList} />

      {/* Bottom sheet (Task 7) */}
    </SafeAreaView>
  )
}
