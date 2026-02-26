import { YStack, Text, H1 } from 'tamagui';

export default function SessionsScreen() {
  return (
    <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$background">
      <H1>Claude View Mobile</H1>
      <Text color="$colorSubtle" marginTop="$2">
        Session monitoring coming soon
      </Text>
    </YStack>
  );
}
