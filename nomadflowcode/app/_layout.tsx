import '@/global.css';

import { NAV_THEME, THEME } from '@/lib/theme';
import { StorageProvider } from '@/lib/context/storage-context';
import { ThemeProvider } from '@react-navigation/native';
import { PortalHost } from '@rn-primitives/portal';
import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { useColorScheme } from 'nativewind';
import {
  useFonts,
  Inter_400Regular,
  Inter_500Medium,
  Inter_600SemiBold,
  Inter_700Bold,
} from '@expo-google-fonts/inter';
import * as SplashScreen from 'expo-splash-screen';
import * as Updates from 'expo-updates';
import { useEffect } from 'react';

export { ErrorBoundary } from 'expo-router';

SplashScreen.preventAutoHideAsync();

export default function RootLayout() {
  const { colorScheme } = useColorScheme();

  const [fontsLoaded] = useFonts({
    Inter_400Regular,
    Inter_500Medium,
    Inter_600SemiBold,
    Inter_700Bold,
  });

  useEffect(() => {
    if (fontsLoaded) {
      SplashScreen.hideAsync();
    }
  }, [fontsLoaded]);

  useEffect(() => {
    if (__DEV__) return;
    async function checkForUpdates() {
      try {
        const update = await Updates.checkForUpdateAsync();
        if (update.isAvailable) {
          await Updates.fetchUpdateAsync();
          await Updates.reloadAsync();
        }
      } catch (e) {
        console.log('Update check failed:', e);
      }
    }
    checkForUpdates();
  }, []);

  if (!fontsLoaded) {
    return null;
  }

  const isDark = colorScheme === 'dark';
  const theme = isDark ? THEME.dark : THEME.light;

  return (
    <StorageProvider>
      <ThemeProvider value={NAV_THEME[colorScheme ?? 'dark']}>
        <StatusBar style={isDark ? 'light' : 'dark'} />
        <Stack
          screenOptions={{
            headerStyle: {
              backgroundColor: theme.card,
            },
            headerTintColor: theme.foreground,
            contentStyle: {
              backgroundColor: theme.background,
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
