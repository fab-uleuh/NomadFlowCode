import { Button } from '@/components/ui/button';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { executeServerCommand } from '@/lib/server-commands';
import type { Repository } from '@shared';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import {
  PackageIcon,
  ChevronRightIcon,
  RefreshCwIcon,
  AlertCircleIcon,
  TerminalIcon,
  GitBranchIcon,
  PlusIcon,
  XIcon,
} from 'lucide-react-native';
import * as React from 'react';
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  FlatList,
  Pressable,
  View,
  ActivityIndicator,
  RefreshControl,
  Alert,
  Modal,
  KeyboardAvoidingView,
  Platform,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

export default function ReposScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const insets = useSafeAreaInsets();
  const params = useLocalSearchParams<{ serverId: string }>();
  const { getServer, addRecentRepo, saveLastSelection, updateServer, lastSelection } = useStorage();

  const server = getServer(params.serverId);

  const [repos, setRepos] = useState<Repository[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [showCloneModal, setShowCloneModal] = useState(false);
  const [cloneUrl, setCloneUrl] = useState('');
  const [cloneToken, setCloneToken] = useState('');
  const [cloneName, setCloneName] = useState('');
  const [isCloning, setIsCloning] = useState(false);

  useEffect(() => {
    if (server) {
      loadRepos();
      saveLastSelection({ serverId: server.id });
      updateServer(server.id, { lastConnected: Date.now() });
    }
  }, [server?.id]);

  const loadRepos = async (isRefresh = false) => {
    if (!server) return;

    if (isRefresh) {
      setIsRefreshing(true);
    } else {
      setIsLoading(true);
    }
    setError(null);

    try {
      const result = await executeServerCommand(server, { action: 'list-repos' });

      if (result.success && result.data) {
        setRepos(result.data.repos);
      } else {
        throw new Error(result.error || t('repos.error.load_failed'));
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : t('common.error.unknown');
      setError(message);
    } finally {
      setIsLoading(false);
      setIsRefreshing(false);
    }
  };

  const handleRepoPress = useCallback(
    (repo: Repository) => {
      if (!server) return;
      addRecentRepo(repo);
      saveLastSelection({ serverId: server.id, repoPath: repo.path });
      router.push({
        pathname: '/features',
        params: { serverId: server.id, repoPath: repo.path },
      });
    },
    [server, router, addRecentRepo, saveLastSelection]
  );

  const handleQuickTerminal = () => {
    if (!server) return;
    Alert.alert(t('repos.quick_terminal.title'), t('repos.quick_terminal.confirm'), [
      { text: t('common.cancel'), style: 'cancel' },
      {
        text: t('common.open'),
        onPress: () => {
          router.push({
            pathname: '/terminal',
            params: {
              serverId: server.id,
              repoPath: '~',
              featureName: 'shell',
            },
          });
        },
      },
    ]);
  };

  const cloneRepo = async () => {
    if (!server) return;

    const trimmedUrl = cloneUrl.trim();
    if (!trimmedUrl) {
      Alert.alert(t('common.error'), t('repos.clone.error.url_required'));
      return;
    }

    setIsCloning(true);

    try {
      const params: Record<string, string> = { url: trimmedUrl };
      if (cloneToken.trim()) params.token = cloneToken.trim();
      if (cloneName.trim()) params.name = cloneName.trim();

      const result = await executeServerCommand(server, {
        action: 'clone-repo',
        params,
      });

      if (result.success && result.data) {
        setShowCloneModal(false);
        setCloneUrl('');
        setCloneToken('');
        setCloneName('');
        await loadRepos();
      } else {
        throw new Error(result.error || t('repos.clone.error.failed'));
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : t('repos.clone.error.failed');
      Alert.alert(t('common.error'), message);
    } finally {
      setIsCloning(false);
    }
  };

  const renderRepo = ({ item }: { item: Repository }) => {
    const isLastUsed = lastSelection.repoPath === item.path;

    return (
      <Pressable onPress={() => handleRepoPress(item)} className="mb-3">
        <Card className={isLastUsed ? 'border-2 border-primary' : ''}>
          <CardHeader className="flex-row items-center gap-3 pb-3">
            <View className="h-10 w-10 items-center justify-center rounded-full bg-primary/10">
              <Icon as={PackageIcon} className="text-primary" size={20} />
            </View>
            <View className="flex-1">
              <View className="flex-row items-center gap-2">
                <CardTitle className="text-base">{item.name}</CardTitle>
                {isLastUsed && (
                  <View className="rounded-full bg-primary px-2 py-0.5">
                    <Text className="text-[10px] font-semibold text-primary-foreground">
                      {t('repos.badge.last_used')}
                    </Text>
                  </View>
                )}
              </View>
              <CardDescription className="text-xs">{item.path}</CardDescription>
              <View className="mt-1 flex-row items-center gap-1">
                <Icon as={GitBranchIcon} className="text-success" size={12} />
                <Text className="text-xs text-success">{item.branch}</Text>
              </View>
            </View>
            <Icon as={ChevronRightIcon} className="text-muted-foreground" size={20} />
          </CardHeader>
        </Card>
      </Pressable>
    );
  };

  const renderEmpty = () => (
    <View className="flex-1 items-center justify-center p-8">
      {error ? (
        <>
          <View className="mb-4 h-20 w-20 items-center justify-center rounded-full bg-destructive/10">
            <Icon as={AlertCircleIcon} className="text-destructive" size={40} />
          </View>
          <Text className="mb-2 text-center text-xl font-bold text-destructive">
            {t('repos.error.connection_title')}
          </Text>
          <Text className="mb-6 text-center text-muted-foreground">{error}</Text>
          <Button onPress={() => loadRepos()}>
            <Icon as={RefreshCwIcon} className="mr-2" size={18} />
            <Text>{t('common.retry')}</Text>
          </Button>
        </>
      ) : (
        <>
          <View className="mb-4 h-20 w-20 items-center justify-center rounded-full bg-muted">
            <Icon as={PackageIcon} className="text-muted-foreground" size={40} />
          </View>
          <Text className="mb-2 text-center text-xl font-bold">{t('repos.empty.title')}</Text>
          <Text className="text-center text-muted-foreground">
            {t('repos.empty.description')}
          </Text>
        </>
      )}
    </View>
  );

  if (!server) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <Text className="text-destructive">{t('common.error.server_not_found')}</Text>
      </View>
    );
  }

  if (isLoading) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <ActivityIndicator size="large" />
        <Text className="mt-4 text-muted-foreground">{t('repos.loading')}</Text>
      </View>
    );
  }

  return (
    <>
      <Stack.Screen
        options={{
          title: server.name,
        }}
      />
      <View className="flex-1 bg-background">
        <FlatList
          data={repos}
          keyExtractor={(item) => item.path}
          renderItem={renderRepo}
          ListEmptyComponent={renderEmpty}
          contentContainerStyle={{
            padding: 16,
            paddingBottom: 100,
            flexGrow: repos.length === 0 ? 1 : undefined,
          }}
          contentInsetAdjustmentBehavior="automatic"
          refreshControl={
            <RefreshControl refreshing={isRefreshing} onRefresh={() => loadRepos(true)} />
          }
          showsVerticalScrollIndicator={false}
        />

        <View className="absolute left-4 right-4 flex-row items-center gap-3" style={{ bottom: insets.bottom + 16 }}>
          <Pressable
            onPress={handleQuickTerminal}
            className="flex-1 flex-row items-center justify-center gap-2 rounded-xl bg-card p-4 shadow-lg">
            <Icon as={TerminalIcon} className="text-primary" size={20} />
            <Text className="font-medium">{t('repos.quick_terminal.button')}</Text>
          </Pressable>
          <Button
            size="icon"
            className="h-14 w-14 rounded-full shadow-lg"
            onPress={() => setShowCloneModal(true)}>
            <Icon as={PlusIcon} size={24} className="text-primary-foreground" />
          </Button>
        </View>

        {/* Clone Repo Modal */}
        <Modal
          visible={showCloneModal}
          transparent
          animationType="fade"
          onRequestClose={() => {
            setShowCloneModal(false);
            setCloneUrl('');
            setCloneToken('');
            setCloneName('');
          }}>
          <KeyboardAvoidingView
            behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
            className="flex-1 items-center justify-center bg-black/70 p-4">
            <Card className="w-full max-w-md">
              <CardHeader className="flex-row items-center justify-between">
                <CardTitle>{t('repos.clone.title')}</CardTitle>
                <Pressable onPress={() => { setShowCloneModal(false); setCloneUrl(''); setCloneToken(''); setCloneName(''); }} hitSlop={12}>
                  <Icon as={XIcon} className="text-muted-foreground" size={20} />
                </Pressable>
              </CardHeader>
              <View className="gap-4 p-4">
                <View className="gap-2">
                  <Label nativeID="cloneUrl">{t('repos.clone.label.url')}</Label>
                  <Input
                    placeholder={t('repos.clone.placeholder.url')}
                    value={cloneUrl}
                    onChangeText={setCloneUrl}
                    autoCapitalize="none"
                    autoCorrect={false}
                    autoFocus
                    keyboardType="url"
                    aria-labelledby="cloneUrl"
                  />
                </View>

                <View className="gap-2">
                  <Label nativeID="cloneToken">{t('repos.clone.label.token')}</Label>
                  <Input
                    placeholder={t('repos.clone.placeholder.token')}
                    value={cloneToken}
                    onChangeText={setCloneToken}
                    autoCapitalize="none"
                    autoCorrect={false}
                    secureTextEntry
                    aria-labelledby="cloneToken"
                  />
                </View>

                <View className="gap-2">
                  <Label nativeID="cloneName">{t('repos.clone.label.name')}</Label>
                  <Input
                    placeholder={t('repos.clone.placeholder.name')}
                    value={cloneName}
                    onChangeText={setCloneName}
                    autoCapitalize="none"
                    autoCorrect={false}
                    aria-labelledby="cloneName"
                  />
                </View>

                <View className="flex-row gap-3 pt-2">
                  <Button
                    variant="outline"
                    className="flex-1"
                    onPress={() => {
                      setShowCloneModal(false);
                      setCloneUrl('');
                      setCloneToken('');
                      setCloneName('');
                    }}>
                    <Text>{t('common.cancel')}</Text>
                  </Button>
                  <Button className="flex-1" onPress={cloneRepo} disabled={isCloning}>
                    {isCloning ? (
                      <ActivityIndicator size="small" color="white" />
                    ) : (
                      <Text>{t('repos.clone.button')}</Text>
                    )}
                  </Button>
                </View>
              </View>
            </Card>
          </KeyboardAvoidingView>
        </Modal>
      </View>
    </>
  );
}
