import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { Stack, useRouter } from 'expo-router';
import {
  BotIcon,
  MoonIcon,
  SunIcon,
  MonitorIcon,
  TrashIcon,
  InfoIcon,
  GlobeIcon,
} from 'lucide-react-native';
import { useColorScheme, colorScheme as nwColorScheme } from 'nativewind';
import * as React from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from '@/lib/i18n';
import { View, ScrollView, Alert, Pressable } from 'react-native';

type AiAgent = 'claude' | 'ollama' | 'custom';

export default function SettingsScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const { colorScheme, setColorScheme } = useColorScheme();
  const { settings, updateSettings, clearAllData } = useStorage();

  const AI_AGENTS: { value: AiAgent; label: string; description: string }[] = [
    { value: 'claude', label: t('settings.ai_agent.claude_label'), description: t('settings.ai_agent.claude_description') },
    { value: 'ollama', label: t('settings.ai_agent.ollama_label'), description: t('settings.ai_agent.ollama_description') },
    { value: 'custom', label: t('settings.ai_agent.custom_label'), description: t('settings.ai_agent.custom_description') },
  ];

  const [customCommand, setCustomCommand] = useState(settings.customAgentCommand || '');
  const [fontSize, setFontSize] = useState(settings.fontSize.toString());

  const handleSave = async () => {
    await updateSettings({
      customAgentCommand: customCommand,
      fontSize: parseInt(fontSize) || 14,
    });
    Alert.alert(t('common.success'), t('settings.saved'));
  };

  const handleClearData = () => {
    Alert.alert(
      t('settings.clear_data.confirm_title'),
      t('settings.clear_data.confirm_message'),
      [
        { text: t('common.cancel'), style: 'cancel' },
        {
          text: t('settings.danger_zone.clear_button'),
          style: 'destructive',
          onPress: async () => {
            await clearAllData();
            router.replace('/');
          },
        },
      ]
    );
  };

  const handleAgentChange = async (agent: AiAgent) => {
    await updateSettings({ defaultAiAgent: agent });
  };

  const handleAutoLaunchToggle = async () => {
    await updateSettings({ autoLaunchAgent: !settings.autoLaunchAgent });
  };

  const handleAutoReconnectToggle = async () => {
    await updateSettings({ autoReconnect: !settings.autoReconnect });
  };

  const cycleTheme = () => {
    const themes: ('light' | 'dark' | 'system')[] = ['light', 'dark', 'system'];
    const currentIndex = themes.indexOf(colorScheme as any);
    const nextTheme = themes[(currentIndex + 1) % themes.length];
    setColorScheme(nextTheme);
  };

  const ThemeIcon = colorScheme === 'dark' ? MoonIcon : colorScheme === 'light' ? SunIcon : MonitorIcon;

  return (
    <>
      <Stack.Screen
        options={{
          title: t('settings.title'),
          headerRight: () => (
            <Button variant="ghost" onPress={handleSave}>
              <Text className="font-semibold text-primary">{t('common.save')}</Text>
            </Button>
          ),
        }}
      />
      <ScrollView className="flex-1 bg-background p-4" contentInsetAdjustmentBehavior="automatic">
        {/* Theme */}
        <Card className="mb-4">
          <CardHeader>
            <CardTitle className="flex-row items-center gap-2">
              <Icon as={ThemeIcon} size={18} />
              <Text className="font-semibold">{t('settings.appearance.title')}</Text>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Pressable
              onPress={cycleTheme}
              className="flex-row items-center justify-between rounded-lg bg-muted p-3">
              <Text>{t('settings.appearance.theme')}</Text>
              <View className="flex-row items-center gap-2">
                <Text className="capitalize text-muted-foreground">{t(`settings.theme.${colorScheme}`)}</Text>
                <Icon as={ThemeIcon} className="text-primary" size={18} />
              </View>
            </Pressable>
            <Pressable
              onPress={() => {
                const langs: ('en' | 'fr')[] = ['en', 'fr'];
                const current = (settings.language || i18n.language || 'en') as 'en' | 'fr';
                const next = langs[(langs.indexOf(current) + 1) % langs.length];
                updateSettings({ language: next });
                i18n.changeLanguage(next);
              }}
              className="flex-row items-center justify-between rounded-lg bg-muted p-3 mt-3">
              <Text>{t('settings.appearance.language')}</Text>
              <View className="flex-row items-center gap-2">
                <Text className="text-muted-foreground">{t(`settings.language.${settings.language || i18n.language || 'en'}`)}</Text>
                <Icon as={GlobeIcon} className="text-primary" size={18} />
              </View>
            </Pressable>
          </CardContent>
        </Card>

        {/* AI Agent */}
        <Card className="mb-4">
          <CardHeader>
            <CardTitle className="flex-row items-center gap-2">
              <Icon as={BotIcon} size={18} />
              <Text className="font-semibold">{t('settings.ai_agent.title')}</Text>
            </CardTitle>
            <CardDescription>{t('settings.ai_agent.description')}</CardDescription>
          </CardHeader>
          <CardContent className="gap-3">
            {AI_AGENTS.map((agent) => (
              <Pressable
                key={agent.value}
                onPress={() => handleAgentChange(agent.value)}
                className={`flex-row items-center justify-between rounded-lg p-3 ${
                  settings.defaultAiAgent === agent.value ? 'bg-primary/10 border border-primary' : 'bg-muted'
                }`}>
                <View>
                  <Text className="font-medium">{agent.label}</Text>
                  <Text className="text-xs text-muted-foreground">{agent.description}</Text>
                </View>
                {settings.defaultAiAgent === agent.value && (
                  <View className="h-4 w-4 rounded-full bg-primary" />
                )}
              </Pressable>
            ))}

            {settings.defaultAiAgent === 'custom' && (
              <View className="mt-2 gap-2">
                <Label nativeID="customCmd">{t('settings.ai_agent.custom_command')}</Label>
                <Input
                  placeholder={t('settings.ai_agent.custom_placeholder')}
                  value={customCommand}
                  onChangeText={setCustomCommand}
                  autoCapitalize="none"
                  autoCorrect={false}
                  aria-labelledby="customCmd"
                />
              </View>
            )}

            <Pressable
              onPress={handleAutoLaunchToggle}
              className="mt-2 flex-row items-center justify-between rounded-lg bg-muted p-3">
              <View>
                <Text className="font-medium">{t('settings.ai_agent.auto_launch')}</Text>
                <Text className="text-xs text-muted-foreground">
                  {t('settings.ai_agent.auto_launch_hint')}
                </Text>
              </View>
              <View
                className={`h-6 w-11 rounded-full p-0.5 ${settings.autoLaunchAgent ? 'bg-primary' : 'bg-muted-foreground/30'}`}>
                <View
                  className={`h-5 w-5 rounded-full bg-white transition-all ${settings.autoLaunchAgent ? 'translate-x-5' : 'translate-x-0'}`}
                />
              </View>
            </Pressable>
          </CardContent>
        </Card>

        {/* Terminal */}
        <Card className="mb-4">
          <CardHeader>
            <CardTitle>{t('settings.terminal.title')}</CardTitle>
            <CardDescription>{t('settings.terminal.description')}</CardDescription>
          </CardHeader>
          <CardContent className="gap-4">
            <View className="gap-2">
              <Label nativeID="fontSize">{t('settings.terminal.font_size')}</Label>
              <Input
                placeholder="14"
                value={fontSize}
                onChangeText={setFontSize}
                keyboardType="number-pad"
                aria-labelledby="fontSize"
              />
            </View>

            <Pressable
              onPress={handleAutoReconnectToggle}
              className="flex-row items-center justify-between rounded-lg bg-muted p-3">
              <View>
                <Text className="font-medium">{t('settings.terminal.auto_reconnect')}</Text>
                <Text className="text-xs text-muted-foreground">
                  {t('settings.terminal.auto_reconnect_hint')}
                </Text>
              </View>
              <View
                className={`h-6 w-11 rounded-full p-0.5 ${settings.autoReconnect ? 'bg-primary' : 'bg-muted-foreground/30'}`}>
                <View
                  className={`h-5 w-5 rounded-full bg-white ${settings.autoReconnect ? 'translate-x-5' : 'translate-x-0'}`}
                />
              </View>
            </Pressable>
          </CardContent>
        </Card>

        {/* About */}
        <Card className="mb-4">
          <CardHeader>
            <CardTitle className="flex-row items-center gap-2">
              <Icon as={InfoIcon} size={18} />
              <Text className="font-semibold">{t('settings.about.title')}</Text>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Text className="text-muted-foreground">
              NomadFlow v1.0.0{'\n'}
              {t('settings.about.description')}
            </Text>
          </CardContent>
        </Card>

        {/* Danger Zone */}
        <Card className="mb-8 border-destructive">
          <CardHeader>
            <CardTitle className="flex-row items-center gap-2 text-destructive">
              <Icon as={TrashIcon} className="text-destructive" size={18} />
              <Text className="font-semibold text-destructive">{t('settings.danger_zone.title')}</Text>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Button variant="destructive" onPress={handleClearData}>
              <Icon as={TrashIcon} className="mr-2" size={18} />
              <Text>{t('settings.danger_zone.clear_button')}</Text>
            </Button>
          </CardContent>
        </Card>
      </ScrollView>
    </>
  );
}
