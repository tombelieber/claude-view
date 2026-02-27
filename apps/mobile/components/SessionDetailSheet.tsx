import { type RelaySession, formatUsd } from '@claude-view/shared'
import { ScrollView } from 'react-native'
import { Separator, Sheet, Text, XStack, YStack } from 'tamagui'

interface Props {
  session: RelaySession | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function SessionDetailSheet({ session, open, onOpenChange }: Props) {
  if (!session) return null

  return (
    <Sheet
      modal
      open={open}
      onOpenChange={onOpenChange}
      snapPoints={[85, 50]}
      dismissOnSnapToBottom
    >
      <Sheet.Overlay backgroundColor="rgba(0,0,0,0.5)" />
      <Sheet.Handle backgroundColor="$gray500" />
      <Sheet.Frame backgroundColor="$gray800" borderTopLeftRadius="$4" borderTopRightRadius="$4">
        <ScrollView style={{ padding: 16 }}>
          {/* Header -- Audit gap #28: use projectDisplayName, not project */}
          <Text color="$gray50" fontWeight="bold" fontSize="$xl">
            {session.projectDisplayName}
          </Text>
          {/* Audit gap #29: model is string | null -- guard against null */}
          <Text color="$gray400" fontSize="$sm" marginTop="$1">
            {session.model ?? 'unknown'}
          </Text>

          {/* Status info */}
          <XStack flexWrap="wrap" marginTop="$4" gap="$4">
            <InfoItem label="Status" value={session.status} />
            <InfoItem label="Model" value={session.model ?? 'unknown'} />
            {/* NOTE (audit fix B1): tokens use camelCase */}
            <InfoItem
              label="Tokens"
              value={`${Math.round((session.tokens.inputTokens + session.tokens.outputTokens) / 1000)}k`}
            />
          </XStack>

          <Separator marginVertical="$4" borderColor="$gray700" />

          {/* Cost */}
          <SectionLabel>Cost</SectionLabel>
          <YStack backgroundColor="$gray900" borderRadius="$3" padding="$3">
            {/* NOTE (audit fix B1): cost is nested object */}
            <CostRow label="Total" value={session.cost.totalUsd} bold />
          </YStack>

          <Separator marginVertical="$4" borderColor="$gray700" />

          {/* Last activity -- NOTE (audit fix B1): field is lastUserMessage */}
          {session.lastUserMessage ? (
            <>
              <SectionLabel>Last Activity</SectionLabel>
              <Text color="$gray200" fontSize="$sm" numberOfLines={4}>
                {session.lastUserMessage}
              </Text>
            </>
          ) : null}

          {/* M2 teaser */}
          <YStack
            marginTop="$6"
            backgroundColor="$gray900"
            borderRadius="$4"
            padding="$4"
            alignItems="center"
            opacity={0.5}
          >
            <Text color="$gray400" fontSize="$sm">
              Approve / Deny -- coming in M2
            </Text>
          </YStack>
        </ScrollView>
      </Sheet.Frame>
    </Sheet>
  )
}

function SectionLabel({ children }: { children: string }) {
  return (
    <Text
      color="$gray400"
      fontSize="$xs"
      textTransform="uppercase"
      letterSpacing={1}
      marginBottom="$2"
    >
      {children}
    </Text>
  )
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <YStack>
      <Text color="$gray400" fontSize="$xs">
        {label}
      </Text>
      <Text color="$gray50" fontSize="$sm">
        {value}
      </Text>
    </YStack>
  )
}

function CostRow({
  label,
  value,
  bold,
}: {
  label: string
  value: number
  bold?: boolean
}) {
  return (
    <XStack justifyContent="space-between" paddingVertical="$1">
      <Text color={bold ? '$gray50' : '$gray400'} fontSize="$sm" fontWeight={bold ? '600' : '400'}>
        {label}
      </Text>
      <Text
        color={bold ? '$gray50' : '$gray400'}
        fontFamily="$mono"
        fontSize="$sm"
        fontWeight={bold ? '600' : '400'}
      >
        {formatUsd(value)}
      </Text>
    </XStack>
  )
}
