import { Button } from '@/components/ui/button';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { executeServerCommand } from '@/lib/server-commands';
import type { Feature } from '@/lib/types';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import {
  GitBranchIcon,
  ChevronRightIcon,
  RefreshCwIcon,
  AlertCircleIcon,
  PlusIcon,
  FolderIcon,
  LeafIcon,
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

export default function FeaturesScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{ serverId: string; repoPath: string }>();
  const { getServer, addRecentFeature, saveLastSelection, lastSelection } = useStorage();

  const server = getServer(params.serverId);
  const repoPath = params.repoPath;
  const repoName = repoPath?.split('/').pop() || 'Repository';

  const [features, setFeatures] = useState<Feature[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [showNewFeatureModal, setShowNewFeatureModal] = useState(false);
  const [newFeatureName, setNewFeatureName] = useState('');
  const [baseBranch, setBaseBranch] = useState('main');

  useEffect(() => {
    if (server) {
      loadFeatures();
      saveLastSelection({ serverId: server.id, repoPath });
    }
  }, [server?.id, repoPath]);

  const loadFeatures = async (isRefresh = false) => {
    if (!server) return;

    if (isRefresh) {
      setIsRefreshing(true);
    } else {
      setIsLoading(true);
    }
    setError(null);

    try {
      const result = await executeServerCommand(server, {
        action: 'list-features',
        params: { repoPath },
      });

      if (result.success && result.data) {
        setFeatures(result.data.features);
      } else {
        throw new Error(result.error || 'Failed to load features');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Erreur inconnue';
      setError(message);
    } finally {
      setIsLoading(false);
      setIsRefreshing(false);
    }
  };

  const handleFeaturePress = useCallback(
    (feature: Feature) => {
      if (!server) return;
      addRecentFeature(feature);
      saveLastSelection({
        serverId: server.id,
        repoPath,
        featureName: feature.name,
      });
      router.push({
        pathname: '/terminal',
        params: {
          serverId: server.id,
          repoPath,
          featureName: feature.name,
        },
      });
    },
    [server, repoPath, router, addRecentFeature, saveLastSelection]
  );

  const createNewFeature = async () => {
    if (!server) return;

    const trimmedName = newFeatureName.trim();
    if (!trimmedName) {
      Alert.alert('Erreur', 'Veuillez entrer un nom de feature');
      return;
    }

    const sanitizedName = trimmedName
      .toLowerCase()
      .replace(/[^a-z0-9-_]/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '');

    if (!sanitizedName) {
      Alert.alert('Erreur', 'Nom de feature invalide');
      return;
    }

    setIsCreating(true);

    try {
      const result = await executeServerCommand(server, {
        action: 'create-feature',
        params: {
          repoPath,
          featureName: sanitizedName,
          baseBranch: baseBranch || 'main',
        },
      });

      if (result.success && result.data) {
        setShowNewFeatureModal(false);
        setNewFeatureName('');

        const newFeature: Feature = {
          name: sanitizedName,
          worktreePath: result.data.worktreePath || `${repoPath}/../worktrees/${sanitizedName}`,
          branch: `feature/${sanitizedName}`,
          isActive: true,
          createdAt: Date.now(),
        };

        addRecentFeature(newFeature);
        router.push({
          pathname: '/terminal',
          params: {
            serverId: server.id,
            repoPath,
            featureName: sanitizedName,
          },
        });
      } else {
        throw new Error(result.error || 'Failed to create feature');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Erreur lors de la création';
      Alert.alert('Erreur', message);
    } finally {
      setIsCreating(false);
    }
  };

  const deleteFeature = async (feature: Feature) => {
    if (!server) return;

    Alert.alert(
      'Supprimer la feature ?',
      `Êtes-vous sûr de vouloir supprimer "${feature.name}" ?\n\nCela supprimera le worktree et la window tmux associée.`,
      [
        { text: 'Annuler', style: 'cancel' },
        {
          text: 'Supprimer',
          style: 'destructive',
          onPress: async () => {
            try {
              await executeServerCommand(server, {
                action: 'delete-feature',
                params: { repoPath, featureName: feature.name },
              });
              loadFeatures();
            } catch (err) {
              Alert.alert('Erreur', 'Impossible de supprimer la feature');
            }
          },
        },
      ]
    );
  };

  const renderFeature = ({ item }: { item: Feature }) => {
    const isLastUsed = lastSelection.featureName === item.name;

    return (
      <Pressable
        onPress={() => handleFeaturePress(item)}
        onLongPress={() => deleteFeature(item)}
        className="mb-3">
        <Card
          className={
            isLastUsed
              ? 'border-2 border-primary'
              : item.isActive
                ? 'border-2 border-success'
                : ''
          }>
          <CardHeader className="flex-row items-center gap-3 pb-3">
            <View
              className={`h-10 w-10 items-center justify-center rounded-full ${item.isActive ? 'bg-success/10' : 'bg-primary/10'}`}>
              <Icon
                as={item.isActive ? LeafIcon : GitBranchIcon}
                className={item.isActive ? 'text-success' : 'text-primary'}
                size={20}
              />
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
                {item.isActive && !isLastUsed && (
                  <View className="rounded-full bg-success px-2 py-0.5">
                    <Text className="text-[10px] font-semibold text-white">Actif</Text>
                  </View>
                )}
              </View>
              <View className="mt-1 flex-row items-center gap-1">
                <Icon as={GitBranchIcon} className="text-muted-foreground" size={12} />
                <CardDescription className="text-xs">{item.branch}</CardDescription>
              </View>
              <View className="mt-1 flex-row items-center gap-1">
                <Icon as={FolderIcon} className="text-muted-foreground" size={12} />
                <CardDescription className="text-xs" numberOfLines={1}>
                  {item.worktreePath}
                </CardDescription>
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
            Erreur de chargement
          </Text>
          <Text className="mb-6 text-center text-muted-foreground">{error}</Text>
          <Button onPress={() => loadFeatures()}>
            <Icon as={RefreshCwIcon} className="mr-2" size={18} />
            <Text>Réessayer</Text>
          </Button>
        </>
      ) : (
        <>
          <View className="mb-4 h-20 w-20 items-center justify-center rounded-full bg-primary/10">
            <Icon as={LeafIcon} className="text-primary" size={40} />
          </View>
          <Text className="mb-2 text-center text-xl font-bold">Aucune feature active</Text>
          <Text className="text-center text-muted-foreground">
            Créez une nouvelle feature pour commencer à développer
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
        <Text className="mt-4 text-muted-foreground">Chargement des features...</Text>
      </View>
    );
  }

  return (
    <>
      <Stack.Screen
        options={{
          title: repoName,
        }}
      />
      <View className="flex-1 bg-background">
        <FlatList
          data={features}
          keyExtractor={(item) => item.name}
          renderItem={renderFeature}
          ListEmptyComponent={renderEmpty}
          contentContainerStyle={{
            padding: 16,
            paddingBottom: 100,
            flexGrow: features.length === 0 ? 1 : undefined,
          }}
          contentInsetAdjustmentBehavior="automatic"
          refreshControl={
            <RefreshControl refreshing={isRefreshing} onRefresh={() => loadFeatures(true)} />
          }
          showsVerticalScrollIndicator={false}
        />

        <Button
          size="icon"
          className="absolute bottom-6 right-6 h-14 w-14 rounded-full shadow-lg"
          onPress={() => setShowNewFeatureModal(true)}>
          <Icon as={PlusIcon} size={24} className="text-primary-foreground" />
        </Button>

        {/* New Feature Modal */}
        <Modal
          visible={showNewFeatureModal}
          transparent
          animationType="fade"
          onRequestClose={() => setShowNewFeatureModal(false)}>
          <KeyboardAvoidingView
            behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
            className="flex-1 items-center justify-center bg-black/70 p-4">
            <Card className="w-full max-w-md">
              <CardHeader className="flex-row items-center justify-between">
                <CardTitle>Nouvelle Feature</CardTitle>
                <Pressable onPress={() => setShowNewFeatureModal(false)}>
                  <Icon as={XIcon} className="text-muted-foreground" size={20} />
                </Pressable>
              </CardHeader>
              <View className="gap-4 p-4">
                <View className="gap-2">
                  <Label nativeID="featureName">Nom de la feature</Label>
                  <Input
                    placeholder="ma-nouvelle-feature"
                    value={newFeatureName}
                    onChangeText={setNewFeatureName}
                    autoCapitalize="none"
                    autoCorrect={false}
                    autoFocus
                    aria-labelledby="featureName"
                  />
                </View>

                <View className="gap-2">
                  <Label nativeID="baseBranch">Branche de base</Label>
                  <Input
                    placeholder="main"
                    value={baseBranch}
                    onChangeText={setBaseBranch}
                    autoCapitalize="none"
                    autoCorrect={false}
                    aria-labelledby="baseBranch"
                  />
                </View>

                <View className="flex-row gap-3 pt-2">
                  <Button
                    variant="outline"
                    className="flex-1"
                    onPress={() => {
                      setShowNewFeatureModal(false);
                      setNewFeatureName('');
                    }}>
                    <Text>Annuler</Text>
                  </Button>
                  <Button className="flex-1" onPress={createNewFeature} disabled={isCreating}>
                    {isCreating ? (
                      <ActivityIndicator size="small" color="white" />
                    ) : (
                      <Text>Créer</Text>
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
