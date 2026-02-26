import { type RelaySession, formatUsd, groupByStatus } from '@claude-view/shared'
import { Text, XStack } from 'tamagui'

export function SummaryBar({ sessions }: { sessions: RelaySession[] }) {
  const { needsYou, autonomous } = groupByStatus(sessions)
  const totalCost = sessions.reduce((sum, s) => sum + s.cost.totalUsd, 0)

  return (
    <XStack
      backgroundColor="$gray800"
      borderTopWidth={1}
      borderTopColor="$gray700"
      paddingHorizontal="$4"
      paddingVertical="$3"
      justifyContent="space-between"
    >
      <Text color="$statusWarning" fontSize="$sm">
        {needsYou.length} needs you
      </Text>
      <Text color="$statusActive" fontSize="$sm">
        {autonomous.length} auto
      </Text>
      <Text color="$gray400" fontFamily="$mono" fontSize="$sm">
        {formatUsd(totalCost)}
      </Text>
    </XStack>
  )
}
