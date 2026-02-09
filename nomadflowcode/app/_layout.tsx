import '@/global.css';

import { NAV_THEME } from '@/lib/theme';
import { StorageProvider } from '@/lib/context/storage-context';
import { ThemeProvider } from '@react-navigation/native';
import { PortalHost } from '@rn-primitives/portal';
import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { useColorScheme } from 'nativewind';

export { ErrorBoundary } from 'expo-router';

export default function RootLayout() {
  const { colorScheme } = useColorScheme();

  return (
    <StorageProvider>
      <ThemeProvider value={NAV_THEME[colorScheme ?? 'dark']}>
        <StatusBar style={colorScheme === 'dark' ? 'light' : 'dark'} />
        <Stack
          screenOptions={{
            headerStyle: {
              backgroundColor: colorScheme === 'dark' ? '#1c1c1e' : '#ffffff',
            },
            headerTintColor: colorScheme === 'dark' ? '#ffffff' : '#000000',
            contentStyle: {
              backgroundColor: colorScheme === 'dark' ? '#000000' : '#f5f5f5',
            },
          }}>
          <Stack.Screen
            name="index"
            options={{
              title: 'Serveurs',
              headerLargeTitle: true,
            }}
          />
          <Stack.Screen
            name="add-server"
            options={{
              title: 'Nouveau Serveur',
              presentation: 'modal',
            }}
          />
          <Stack.Screen
            name="repos"
            options={{
              title: 'Repositories',
            }}
          />
          <Stack.Screen
            name="features"
            options={{
              title: 'Features',
            }}
          />
          <Stack.Screen
            name="terminal"
            options={{
              headerShown: false,
              presentation: 'fullScreenModal',
            }}
          />
          <Stack.Screen
            name="settings"
            options={{
              title: 'Paramètres',
              presentation: 'modal',
            }}
          />
        </Stack>
        <PortalHost />
      </ThemeProvider>
    </StorageProvider>
  );
}
