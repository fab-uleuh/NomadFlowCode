import { Button } from '@/components/ui/button';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { executeServerCommand } from '@/lib/server-commands';
import type { Repository } from '@/lib/types';
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

export default function ReposScreen() {
  const router = useRouter();
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
        throw new Error(result.error || 'Failed to load repos');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Erreur inconnue';
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
    Alert.alert('Terminal direct', 'Ouvrir un terminal sans sélectionner de feature ?', [
      { text: 'Annuler', style: 'cancel' },
      {
        text: 'Ouvrir',
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
      Alert.alert('Erreur', 'Veuillez entrer une URL de repository');
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
        throw new Error(result.error || 'Failed to clone repository');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Erreur lors du clone';
      Alert.alert('Erreur', message);
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
                      Dernier
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
            Erreur de connexion
          </Text>
          <Text className="mb-6 text-center text-muted-foreground">{error}</Text>
          <Button onPress={() => loadRepos()}>
            <Icon as={RefreshCwIcon} className="mr-2" size={18} />
            <Text>Réessayer</Text>
          </Button>
        </>
      ) : (
        <>
          <View className="mb-4 h-20 w-20 items-center justify-center rounded-full bg-muted">
            <Icon as={PackageIcon} className="text-muted-foreground" size={40} />
          </View>
          <Text className="mb-2 text-center text-xl font-bold">Aucun repository trouvé</Text>
          <Text className="text-center text-muted-foreground">
            Vérifiez que le script list-repos.sh est bien configuré sur le serveur
          </Text>
        </>
      )}
    </View>
  );

  if (!server) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <Text className="text-destructive">Serveur non trouvé</Text>
      </View>
    );
  }

  if (isLoading) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <ActivityIndicator size="large" />
        <Text className="mt-4 text-muted-foreground">Chargement des repositories...</Text>
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

        <View className="absolute bottom-6 left-4 right-4 flex-row items-center gap-3">
          <Pressable
            onPress={handleQuickTerminal}
            className="flex-1 flex-row items-center justify-center gap-2 rounded-xl bg-card p-4 shadow-lg">
            <Icon as={TerminalIcon} className="text-primary" size={20} />
            <Text className="font-medium">Terminal rapide</Text>
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
          onRequestClose={() => setShowCloneModal(false)}>
          <KeyboardAvoidingView
            behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
            className="flex-1 items-center justify-center bg-black/70 p-4">
            <Card className="w-full max-w-md">
              <CardHeader className="flex-row items-center justify-between">
                <CardTitle>Cloner un repository</CardTitle>
                <Pressable onPress={() => setShowCloneModal(false)}>
                  <Icon as={XIcon} className="text-muted-foreground" size={20} />
                </Pressable>
              </CardHeader>
              <View className="gap-4 p-4">
                <View className="gap-2">
                  <Label nativeID="cloneUrl">URL (HTTPS)</Label>
                  <Input
                    placeholder="https://github.com/user/repo.git"
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
                  <Label nativeID="cloneToken">Token (optionnel)</Label>
                  <Input
                    placeholder="ghp_... / glpat-... / token"
                    value={cloneToken}
                    onChangeText={setCloneToken}
                    autoCapitalize="none"
                    autoCorrect={false}
                    secureTextEntry
                    aria-labelledby="cloneToken"
                  />
                </View>

                <View className="gap-2">
                  <Label nativeID="cloneName">Nom (optionnel)</Label>
                  <Input
                    placeholder="Nom du dossier (auto-détecté)"
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
                    <Text>Annuler</Text>
                  </Button>
                  <Button className="flex-1" onPress={cloneRepo} disabled={isCloning}>
                    {isCloning ? (
                      <ActivityIndicator size="small" color="white" />
                    ) : (
                      <Text>Cloner</Text>
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
