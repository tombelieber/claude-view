import { type RelaySession, formatUsd } from '@claude-view/shared'
import { FileText } from 'lucide-react-native'
import { Pressable } from 'react-native'
import { Circle, Text, XStack, YStack } from 'tamagui'

// NOTE (audit fix B1): Rust status is 'working'|'paused'|'done', not 'active'|'waiting'|'idle'
// Audit gap #20: Use Tamagui tokens, not hardcoded hex
const STATUS_COLORS: Record<string, string> = {
  working: '$statusActive',
  paused: '$statusWarning',
  done: '$gray500',
}

interface Props {
  session: RelaySession
  onPress: () => void
}

export function SessionCard({ session, onPress }: Props) {
  const statusColor = STATUS_COLORS[session.status] ?? '$gray500'

  return (
    <Pressable onPress={onPress} style={({ pressed }) => ({ opacity: pressed ? 0.8 : 1 })}>
      <YStack bg="$gray800" rounded="$4" p="$4" mb="$2">
        {/* Audit gap #28: Use projectDisplayName, NOT project */}
        <Text color="$gray50" fontWeight="600" fontSize="$base">
          {session.projectDisplayName}
        </Text>
        <XStack items="center" gap="$2" mt="$1">
          <Circle size={6} bg={statusColor as any} />
          <Text color="$gray400" fontSize="$sm">
            {session.status}
          </Text>
          <Text color="$gray500" fontSize="$sm">
            ·
          </Text>
          <Text color="$gray400" fontSize="$sm" fontFamily="$mono">
            {/* Audit gap #5: model is string | null — guard against null */}
            {session.model ?? 'unknown'}
          </Text>
        </XStack>
        {/* Last user message */}
        {session.lastUserMessage ? (
          <Text color="$gray200" fontSize="$sm" numberOfLines={2} mt="$2">
            {session.lastUserMessage}
          </Text>
        ) : null}
        {/* IDE file chip */}
        {session.lastUserFile ? (
          <XStack items="center" gap="$1" mt="$1">
            <FileText size={12} color="$gray400" />
            <Text color="$gray400" fontSize="$xs" fontFamily="$mono">
              {session.lastUserFile}
            </Text>
          </XStack>
        ) : null}
        {/* NOTE (audit fix B1): cost is nested object, tokens use camelCase */}
        <XStack justify="space-between" items="center" mt="$3">
          <Text color="$gray400" fontFamily="$mono" fontSize="$sm">
            {formatUsd(session.cost.totalUsd)}
          </Text>
          <Text color="$gray500" fontSize="$xs">
            {session.tokens.inputTokens + session.tokens.outputTokens} tokens
          </Text>
        </XStack>
      </YStack>
    </Pressable>
  )
}
