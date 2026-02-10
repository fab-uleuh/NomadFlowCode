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
} from 'lucide-react-native';
import { useColorScheme, colorScheme as nwColorScheme } from 'nativewind';
import * as React from 'react';
import { useState } from 'react';
import { View, ScrollView, Alert, Pressable } from 'react-native';

type AiAgent = 'claude' | 'ollama' | 'custom';

const AI_AGENTS: { value: AiAgent; label: string; description: string }[] = [
  { value: 'claude', label: 'Claude', description: 'Claude CLI (ask-claude)' },
  { value: 'ollama', label: 'Ollama', description: 'deepseek-coder local' },
  { value: 'custom', label: 'Autre', description: 'Saisir une commande agent' },
];

export default function SettingsScreen() {
  const router = useRouter();
  const { colorScheme, setColorScheme } = useColorScheme();
  const { settings, updateSettings, clearAllData } = useStorage();

  const [customCommand, setCustomCommand] = useState(settings.customAgentCommand || '');
  const [fontSize, setFontSize] = useState(settings.fontSize.toString());
  const [tmuxPrefix, setTmuxPrefix] = useState(settings.tmuxSessionPrefix);

  const handleSave = async () => {
    await updateSettings({
      customAgentCommand: customCommand,
      fontSize: parseInt(fontSize) || 14,
      tmuxSessionPrefix: tmuxPrefix,
    });
    Alert.alert('Succès', 'Paramètres enregistrés');
  };

  const handleClearData = () => {
    Alert.alert(
      'Effacer toutes les données ?',
      'Cette action supprimera tous vos serveurs, historique et paramètres. Cette action est irréversible.',
      [
        { text: 'Annuler', style: 'cancel' },
        {
          text: 'Effacer',
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
          title: 'Paramètres',
          headerRight: () => (
            <Button variant="ghost" onPress={handleSave}>
              <Text className="font-semibold text-primary">Enregistrer</Text>
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
              <Text className="font-semibold">Apparence</Text>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Pressable
              onPress={cycleTheme}
              className="flex-row items-center justify-between rounded-lg bg-muted p-3">
              <Text>Thème</Text>
              <View className="flex-row items-center gap-2">
                <Text className="capitalize text-muted-foreground">{colorScheme}</Text>
                <Icon as={ThemeIcon} className="text-primary" size={18} />
              </View>
            </Pressable>
          </CardContent>
        </Card>

        {/* AI Agent */}
        <Card className="mb-4">
          <CardHeader>
            <CardTitle className="flex-row items-center gap-2">
              <Icon as={BotIcon} size={18} />
              <Text className="font-semibold">Agent IA</Text>
            </CardTitle>
            <CardDescription>Configure l'assistant IA lancé automatiquement</CardDescription>
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
                <Label nativeID="customCmd">Commande personnalisée</Label>
                <Input
                  placeholder="nvim, cursor, etc."
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
                <Text className="font-medium">Lancement automatique</Text>
                <Text className="text-xs text-muted-foreground">
                  Lance l'agent IA à l'ouverture du terminal
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
            <CardTitle>Terminal</CardTitle>
            <CardDescription>Paramètres du terminal</CardDescription>
          </CardHeader>
          <CardContent className="gap-4">
            <View className="gap-2">
              <Label nativeID="fontSize">Taille de police</Label>
              <Input
                placeholder="14"
                value={fontSize}
                onChangeText={setFontSize}
                keyboardType="number-pad"
                aria-labelledby="fontSize"
              />
            </View>

            <View className="gap-2">
              <Label nativeID="tmuxPrefix">Préfixe session tmux</Label>
              <Input
                placeholder="nomadflow"
                value={tmuxPrefix}
                onChangeText={setTmuxPrefix}
                autoCapitalize="none"
                autoCorrect={false}
                aria-labelledby="tmuxPrefix"
              />
            </View>

            <Pressable
              onPress={handleAutoReconnectToggle}
              className="flex-row items-center justify-between rounded-lg bg-muted p-3">
              <View>
                <Text className="font-medium">Reconnexion automatique</Text>
                <Text className="text-xs text-muted-foreground">
                  Reconnecte automatiquement en cas de déconnexion
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
              <Text className="font-semibold">À propos</Text>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Text className="text-muted-foreground">
              NomadFlow v1.0.0{'\n'}
              Terminal mobile résilient avec assistant IA
            </Text>
          </CardContent>
        </Card>

        {/* Danger Zone */}
        <Card className="mb-8 border-destructive">
          <CardHeader>
            <CardTitle className="flex-row items-center gap-2 text-destructive">
              <Icon as={TrashIcon} className="text-destructive" size={18} />
              <Text className="font-semibold text-destructive">Zone dangereuse</Text>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Button variant="destructive" onPress={handleClearData}>
              <Icon as={TrashIcon} className="mr-2" size={18} />
              <Text>Effacer toutes les données</Text>
            </Button>
          </CardContent>
        </Card>
      </ScrollView>
    </>
  );
}
