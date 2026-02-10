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
  const params = useLocalSearchParams<{ serverId?: string; url?: string; secret?: string }>();
  const { addServer, updateServer, getServer } = useStorage();

  const [name, setName] = useState('');
  const [apiUrl, setApiUrl] = useState('');
  const [ttydUrl, setTtydUrl] = useState('');
  const [authToken, setAuthToken] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const isEditing = !!params.serverId;
  const existingServer = params.serverId ? getServer(params.serverId) : undefined;

  useEffect(() => {
    if (existingServer) {
      setName(existingServer.name);
      setApiUrl(existingServer.apiUrl || '');
      setTtydUrl(existingServer.ttydUrl || '');
      setAuthToken(existingServer.authToken || '');
    }
  }, [existingServer]);

  // Pre-fill from deep link: nomadflowcode://connect?url=...&secret=...
  useEffect(() => {
    if (params.url && !isEditing) {
      handleApiUrlChange(params.url);
      if (params.secret) setAuthToken(params.secret);
      // Auto-generate a name from the URL hostname
      try {
        const hostname = new URL(params.url).hostname;
        setName(hostname.split('.')[0]);
      } catch {
        // ignore
      }
    }
  }, [params.url, params.secret]);

  /** Auto-derive ttydUrl from apiUrl: {apiUrl}/terminal (proxied through the server). */
  const handleApiUrlChange = (value: string) => {
    setApiUrl(value);
    try {
      const url = new URL(value);
      // Terminal is now proxied through the API server at /terminal
      url.pathname = '/terminal';
      setTtydUrl(url.toString().replace(/\/$/, ''));
    } catch {
      // Keep current ttydUrl if apiUrl is not yet valid
    }
  };

  const handleSubmit = async () => {
    if (!name.trim()) {
      Alert.alert('Erreur', 'Veuillez entrer un nom pour le serveur');
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
        apiUrl: apiUrl.trim(),
        ttydUrl: ttydUrl.trim() || undefined,
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
                <Label nativeID="apiUrl">URL API</Label>
                <Input
                  placeholder="http://192.168.1.100:8080"
                  value={apiUrl}
                  onChangeText={handleApiUrlChange}
                  autoCapitalize="none"
                  autoCorrect={false}
                  keyboardType="url"
                  aria-labelledby="apiUrl"
                />
                <Text className="text-xs text-muted-foreground">
                  L'URL de l'API NomadFlow (port 8080 par défaut).
                </Text>
              </View>

              <View className="gap-2">
                <Label nativeID="ttydUrl">URL Terminal (ttyd)</Label>
                <Input
                  placeholder="http://192.168.1.100:8080/terminal"
                  value={ttydUrl}
                  onChangeText={setTtydUrl}
                  autoCapitalize="none"
                  autoCorrect={false}
                  keyboardType="url"
                  aria-labelledby="ttydUrl"
                />
                <Text className="text-xs text-muted-foreground">
                  Se remplit automatiquement depuis l'URL API (/terminal par défaut).
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
