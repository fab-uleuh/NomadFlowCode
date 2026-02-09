import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import * as React from 'react';
import { useState, useEffect } from 'react';
import { View, ScrollView, KeyboardAvoidingView, Platform, Alert } from 'react-native';

export default function AddServerScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{ serverId?: string }>();
  const { addServer, updateServer, getServer } = useStorage();

  const [name, setName] = useState('');
  const [ttydUrl, setTtydUrl] = useState('');
  const [apiUrl, setApiUrl] = useState('');
  const [authToken, setAuthToken] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const isEditing = !!params.serverId;
  const existingServer = params.serverId ? getServer(params.serverId) : undefined;

  // Auto-generate API URL from ttyd URL
  const deriveApiUrl = (ttydUrlValue: string): string => {
    try {
      const parsedUrl = new URL(ttydUrlValue);
      const protocol = parsedUrl.protocol;
      // Default API port is 8080, ttyd default is 7681
      const port = parsedUrl.port === '7681' ? '8080' : parsedUrl.port;
      return `${protocol}//${parsedUrl.hostname}:${port}`;
    } catch {
      return '';
    }
  };

  useEffect(() => {
    if (existingServer) {
      setName(existingServer.name);
      setTtydUrl(existingServer.ttydUrl || '');
      setApiUrl(existingServer.apiUrl || deriveApiUrl(existingServer.ttydUrl || ''));
      setAuthToken(existingServer.authToken || '');
    }
  }, [existingServer]);

  // Update API URL when ttyd URL changes (auto-generate if empty or matches derived pattern)
  const handleTtydUrlChange = (text: string) => {
    setTtydUrl(text);
    const derived = deriveApiUrl(text);
    // Auto-update API URL if it's empty or was previously auto-generated
    if (derived && (!apiUrl || apiUrl === deriveApiUrl(ttydUrl))) {
      setApiUrl(derived);
    }
  };

  const handleSubmit = async () => {
    if (!name.trim()) {
      Alert.alert('Erreur', 'Veuillez entrer un nom pour le serveur');
      return;
    }

    if (!ttydUrl.trim()) {
      Alert.alert('Erreur', 'Veuillez entrer une URL ttyd');
      return;
    }

    // Basic URL validation
    if (!ttydUrl.startsWith('http://') && !ttydUrl.startsWith('https://')) {
      Alert.alert('Erreur', "L'URL ttyd doit commencer par http:// ou https://");
      return;
    }

    if (!apiUrl.trim()) {
      Alert.alert('Erreur', 'Veuillez entrer une URL API');
      return;
    }

    if (!apiUrl.startsWith('http://') && !apiUrl.startsWith('https://')) {
      Alert.alert('Erreur', "L'URL API doit commencer par http:// ou https://");
      return;
    }

    setIsSubmitting(true);

    try {
      const serverData = {
        name: name.trim(),
        ttydUrl: ttydUrl.trim(),
        apiUrl: apiUrl.trim(),
        authToken: authToken.trim() || undefined,
      };

      if (isEditing && params.serverId) {
        await updateServer(params.serverId, serverData);
      } else {
        await addServer(serverData);
      }
      router.back();
    } catch (error) {
      Alert.alert('Erreur', 'Impossible de sauvegarder le serveur');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <>
      <Stack.Screen
        options={{
          title: isEditing ? 'Modifier le serveur' : 'Nouveau serveur',
          headerRight: () => (
            <Button variant="ghost" onPress={handleSubmit} disabled={isSubmitting}>
              <Text className="text-primary font-semibold">
                {isSubmitting ? 'Enregistrement...' : 'Enregistrer'}
              </Text>
            </Button>
          ),
        }}
      />
      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
        className="flex-1 bg-background">
        <ScrollView className="flex-1 p-4" keyboardShouldPersistTaps="handled">
          <Card>
            <CardHeader>
              <CardTitle>Configuration du serveur</CardTitle>
            </CardHeader>
            <CardContent className="gap-4">
              <View className="gap-2">
                <Label nativeID="name">Nom du serveur</Label>
                <Input
                  placeholder="Mon serveur de dev"
                  value={name}
                  onChangeText={setName}
                  autoCapitalize="words"
                  aria-labelledby="name"
                />
              </View>

              <View className="gap-2">
                <Label nativeID="ttydUrl">URL ttyd</Label>
                <Input
                  placeholder="http://192.168.1.100:7681"
                  value={ttydUrl}
                  onChangeText={handleTtydUrlChange}
                  autoCapitalize="none"
                  autoCorrect={false}
                  keyboardType="url"
                  aria-labelledby="ttydUrl"
                />
                <Text className="text-xs text-muted-foreground">
                  L'URL HTTP du terminal ttyd (port 7681 par défaut)
                </Text>
              </View>

              <View className="gap-2">
                <Label nativeID="apiUrl">URL API</Label>
                <Input
                  placeholder="http://192.168.1.100:8080"
                  value={apiUrl}
                  onChangeText={setApiUrl}
                  autoCapitalize="none"
                  autoCorrect={false}
                  keyboardType="url"
                  aria-labelledby="apiUrl"
                />
                <Text className="text-xs text-muted-foreground">
                  L'URL de l'API NomadFlow (port 8080 par défaut, auto-générée)
                </Text>
              </View>

              <View className="gap-2">
                <Label nativeID="token">Secret d'authentification (optionnel)</Label>
                <Input
                  placeholder="Secret partagé"
                  value={authToken}
                  onChangeText={setAuthToken}
                  autoCapitalize="none"
                  autoCorrect={false}
                  secureTextEntry
                  aria-labelledby="token"
                />
                <Text className="text-xs text-muted-foreground">
                  Ce secret protège l'API et le terminal. Doit correspondre au secret dans config.toml du serveur.
                </Text>
              </View>
            </CardContent>
          </Card>

          <View className="mt-6 gap-3">
            <Button onPress={handleSubmit} disabled={isSubmitting}>
              <Text>{isSubmitting ? 'Enregistrement...' : isEditing ? 'Mettre à jour' : 'Ajouter le serveur'}</Text>
            </Button>

            <Button variant="outline" onPress={() => router.back()}>
              <Text>Annuler</Text>
            </Button>
          </View>
        </ScrollView>
      </KeyboardAvoidingView>
    </>
  );
}
