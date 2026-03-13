import { type LiveSession, formatUsd } from '@claude-view/shared'
import { FileText } from 'lucide-react-native'
import { ScrollView } from 'react-native'
import { Separator, Sheet, Text, XStack, YStack } from 'tamagui'

interface Props {
  session: LiveSession | null
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
      <Sheet.Overlay bg="rgba(0,0,0,0.5)" />
      <Sheet.Handle bg="$gray500" />
      <Sheet.Frame bg="$gray800" borderTopLeftRadius="$4" borderTopRightRadius="$4">
        <ScrollView style={{ padding: 16 }}>
          {/* Header -- Audit gap #28: use projectDisplayName, not project */}
          <Text color="$gray50" fontWeight="bold" fontSize="$xl">
            {session.projectDisplayName}
          </Text>
          {/* Audit gap #29: model is string | null -- guard against null */}
          <Text color="$gray400" fontSize="$sm" mt="$1">
            {session.model ?? 'unknown'}
          </Text>

          {/* Status info */}
          <XStack flexWrap="wrap" mt="$4" gap="$4">
            <InfoItem label="Status" value={session.status} />
            <InfoItem label="Model" value={session.model ?? 'unknown'} />
            {/* NOTE (audit fix B1): tokens use camelCase */}
            <InfoItem
              label="Tokens"
              value={`${Math.round((session.tokens.inputTokens + session.tokens.outputTokens) / 1000)}k`}
            />
          </XStack>

          <Separator my="$4" borderColor="$gray700" />

          {/* Cost */}
          <SectionLabel>Cost</SectionLabel>
          <YStack bg="$gray900" rounded="$3" p="$3">
            {/* NOTE (audit fix B1): cost is nested object */}
            <CostRow label="Total" value={session.cost.totalUsd} bold />
          </YStack>

          <Separator my="$4" borderColor="$gray700" />

          {/* Last activity -- NOTE (audit fix B1): field is lastUserMessage */}
          {session.lastUserMessage ? (
            <>
              <SectionLabel>Last Activity</SectionLabel>
              <Text color="$gray200" fontSize="$sm" numberOfLines={4}>
                {session.lastUserMessage}
              </Text>
              {session.userFiles && session.userFiles.length > 0 ? (
                <XStack items="center" gap="$1" mt="$2">
                  <FileText size={14} color="$gray400" />
                  <Text color="$gray400" fontSize="$xs" fontFamily="$mono">
                    Viewing: {session.userFiles[0].displayName}
                  </Text>
                </XStack>
              ) : null}
            </>
          ) : null}

          {/* M2 teaser */}
          <YStack mt="$6" bg="$gray900" rounded="$4" p="$4" items="center" opacity={0.5}>
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
    <Text color="$gray400" fontSize="$xs" textTransform="uppercase" letterSpacing={1} mb="$2">
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
    <XStack justify="space-between" py="$1">
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
