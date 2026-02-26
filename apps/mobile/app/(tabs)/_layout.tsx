import { Tabs } from 'expo-router';

export default function TabLayout() {
  return (
    <Tabs>
      <Tabs.Screen
        name="index"
        options={{ title: 'Sessions', headerTitle: 'Claude Sessions' }}
      />
    </Tabs>
  );
}
