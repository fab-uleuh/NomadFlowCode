import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { View, ScrollView, KeyboardAvoidingView, Platform, Alert } from 'react-native';

export default function AddServerScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const params = useLocalSearchParams<{ serverId?: string; url?: string; secret?: string }>();
  const { addServer, updateServer, getServer } = useStorage();

  const [name, setName] = useState('');
  const [apiUrl, setApiUrl] = useState('');
  const [authToken, setAuthToken] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const hasPreFilled = useRef(false);

  const isEditing = !!params.serverId;
  const existingServer = params.serverId ? getServer(params.serverId) : undefined;

  useEffect(() => {
    if (existingServer) {
      setName(existingServer.name);
      setApiUrl(existingServer.apiUrl || '');
      setAuthToken(existingServer.authToken || '');
    }
  }, [existingServer]);

  // Pre-fill from deep link: nomadflowcode://add-server?url=...&secret=...
  // Only pre-fill once to avoid overwriting user edits on re-render
  useEffect(() => {
    if (params.url && !isEditing && !hasPreFilled.current) {
      hasPreFilled.current = true;
      setApiUrl(params.url);
      if (params.secret) setAuthToken(params.secret);
      // Auto-generate a name from the URL hostname (string ops, no new URL())
      const match = params.url.match(/^https?:\/\/([^:/]+)/);
      if (match) setName(match[1].split('.')[0]);
    }
  }, [params.url, params.secret, isEditing]);

  const handleSubmit = async () => {
    if (!name.trim()) {
      Alert.alert(t('common.error'), t('servers.add.error.name_required'));
      return;
    }

    if (!apiUrl.trim()) {
      Alert.alert(t('common.error'), t('servers.add.error.url_required'));
      return;
    }

    if (!apiUrl.startsWith('http://') && !apiUrl.startsWith('https://')) {
      Alert.alert(t('common.error'), t('servers.add.error.url_invalid_protocol'));
      return;
    }

    setIsSubmitting(true);

    try {
      const serverData = {
        name: name.trim(),
        apiUrl: apiUrl.trim(),
        authToken: authToken.trim() || undefined,
      };

      if (isEditing && params.serverId) {
        await updateServer(params.serverId, serverData);
      } else {
        await addServer(serverData);
      }
      router.replace('/');
    } catch (error) {
      Alert.alert(t('common.error'), t('servers.add.error.save_failed'));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <>
      <Stack.Screen
        options={{
          title: isEditing ? t('servers.edit.title') : t('servers.add.title'),
          headerRight: () => (
            <Button variant="ghost" onPress={handleSubmit} disabled={isSubmitting}>
              <Text className="text-primary font-semibold">
                {isSubmitting ? t('common.saving') : isEditing ? t('common.update') : t('common.save')}
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
              <CardTitle>{t('servers.add.section_title')}</CardTitle>
            </CardHeader>
            <CardContent className="gap-4">
              <View className="gap-2">
                <Label nativeID="name">{t('servers.add.label.name')}</Label>
                <Input
                  placeholder={t('servers.add.placeholder.name')}
                  value={name}
                  onChangeText={setName}
                  autoCapitalize="words"
                  aria-labelledby="name"
                />
              </View>

              <View className="gap-2">
                <Label nativeID="apiUrl">{t('servers.add.label.url')}</Label>
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
                  {t('servers.add.hint.url')}
                </Text>
              </View>

              <View className="gap-2">
                <Label nativeID="token">{t('servers.add.label.secret')}</Label>
                <Input
                  placeholder={t('servers.add.placeholder.secret')}
                  value={authToken}
                  onChangeText={setAuthToken}
                  autoCapitalize="none"
                  autoCorrect={false}
                  secureTextEntry
                  aria-labelledby="token"
                />
                <Text className="text-xs text-muted-foreground">
                  {t('servers.add.hint.secret')}
                </Text>
              </View>
            </CardContent>
          </Card>

          <View className="mt-6 gap-3">
            <Button onPress={handleSubmit} disabled={isSubmitting}>
              <Text>{isSubmitting ? t('common.saving') : isEditing ? t('common.update') : t('servers.add.submit')}</Text>
            </Button>

            <Button variant="outline" onPress={() => router.canGoBack() ? router.back() : router.replace('/')}>
              <Text>{t('common.cancel')}</Text>
            </Button>
          </View>
        </ScrollView>
      </KeyboardAvoidingView>
    </>
  );
}
