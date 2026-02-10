import { Button } from '@/components/ui/button';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { executeServerCommand } from '@/lib/server-commands';
import type { Feature, BranchInfo } from '@shared';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import {
  GitBranchIcon,
  ChevronRightIcon,
  RefreshCwIcon,
  AlertCircleIcon,
  PlusIcon,
  FolderIcon,
  HomeIcon,
  LeafIcon,
  XIcon,
  LinkIcon,
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

type ModalTab = 'new' | 'existing';

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

  // Modal state
  const [showModal, setShowModal] = useState(false);
  const [activeTab, setActiveTab] = useState<ModalTab>('new');

  // New branch tab state
  const [branchName, setBranchName] = useState('');
  const [baseBranch, setBaseBranch] = useState('main');

  // Branch list state (shared between tabs)
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [allBranches, setAllBranches] = useState<BranchInfo[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedBranch, setSelectedBranch] = useState<string | null>(null);
  const [isLoadingBranches, setIsLoadingBranches] = useState(false);

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

  const loadBranches = async () => {
    if (!server) return;
    setIsLoadingBranches(true);

    try {
      const result = await executeServerCommand(server, {
        action: 'list-branches',
        params: { repoPath },
      });

      if (result.success && result.data) {
        const defaultBr = result.data.defaultBranch || 'main';
        setBranches(result.data.branches);
        setBaseBranch(defaultBr);

        // Build full list for base branch picker (includes worktree branches)
        const seen = new Set<string>();
        const all: BranchInfo[] = [];
        // Add worktree branches first (they are valid base branches)
        features.forEach((f) => {
          if (f.branch && !seen.has(f.branch)) {
            seen.add(f.branch);
            all.push({ name: f.branch, isRemote: false });
          }
        });
        // Add available branches
        result.data.branches.forEach((b: BranchInfo) => {
          if (!seen.has(b.name)) {
            seen.add(b.name);
            all.push(b);
          }
        });
        setAllBranches(all);
      }
    } catch {
      // Silently fail — branches list is optional
    } finally {
      setIsLoadingBranches(false);
    }
  };

  const openModal = () => {
    setShowModal(true);
    setActiveTab('new');
    setBranchName('');
    setSearchQuery('');
    setSelectedBranch(null);
    setBranches([]);
    setAllBranches([]);
    loadBranches();
  };

  const closeModal = () => {
    setShowModal(false);
    setBranchName('');
    setSearchQuery('');
    setSelectedBranch(null);
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

  const navigateToTerminal = (featureName: string, worktreePath: string, branch: string) => {
    if (!server) return;

    const newFeature: Feature = {
      name: featureName,
      worktreePath,
      branch,
      isActive: true,
      createdAt: Date.now(),
    };

    addRecentFeature(newFeature);
    router.push({
      pathname: '/terminal',
      params: {
        serverId: server.id,
        repoPath,
        featureName,
      },
    });
  };

  const createNewBranch = async () => {
    if (!server) return;

    const trimmedName = branchName.trim();
    if (!trimmedName) {
      Alert.alert('Erreur', 'Veuillez entrer un nom de branche');
      return;
    }

    // Allow slashes for branch prefixes (feature/, bugfix/, etc.)
    const sanitizedName = trimmedName
      .replace(/[^a-zA-Z0-9-_/.]/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '');

    if (!sanitizedName) {
      Alert.alert('Erreur', 'Nom de branche invalide');
      return;
    }

    setIsCreating(true);

    try {
      const result = await executeServerCommand(server, {
        action: 'create-feature',
        params: {
          repoPath,
          branchName: sanitizedName,
          baseBranch: baseBranch || 'main',
        },
      });

      if (result.success && result.data) {
        closeModal();
        // Derive feature name from worktree path (last segment)
        const featureName = result.data.worktreePath.split('/').pop() || sanitizedName;
        navigateToTerminal(featureName, result.data.worktreePath, result.data.branch);
      } else {
        throw new Error(result.error || 'Failed to create branch');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Erreur lors de la creation';
      Alert.alert('Erreur', message);
    } finally {
      setIsCreating(false);
    }
  };

  const attachExistingBranch = async () => {
    if (!server || !selectedBranch) return;

    setIsCreating(true);

    try {
      const result = await executeServerCommand(server, {
        action: 'attach-branch',
        params: {
          repoPath,
          branchName: selectedBranch,
        },
      });

      if (result.success && result.data) {
        closeModal();
        const featureName = result.data.worktreePath.split('/').pop() || selectedBranch;
        navigateToTerminal(featureName, result.data.worktreePath, result.data.branch);
      } else {
        throw new Error(result.error || 'Failed to attach branch');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Erreur lors de l\'attachement';
      Alert.alert('Erreur', message);
    } finally {
      setIsCreating(false);
    }
  };

  const deleteFeature = async (feature: Feature) => {
    if (!server) return;
    if (feature.isMain) return;

    Alert.alert(
      'Supprimer la feature ?',
      `Etes-vous sur de vouloir supprimer "${feature.name}" ?\n\nCela supprimera le worktree et la window tmux associee.`,
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
            } catch {
              Alert.alert('Erreur', 'Impossible de supprimer la feature');
            }
          },
        },
      ]
    );
  };

  // For "existing branch" tab: available branches (not in worktrees)
  const filteredBranches = branches.filter((b) =>
    b.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // For "new branch" tab: all branches (including worktree ones) as base
  const filteredBaseBranches = allBranches.filter((b) =>
    b.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const renderFeature = ({ item }: { item: Feature }) => {
    const isLastUsed = lastSelection.featureName === item.name;

    const iconAs = item.isMain ? HomeIcon : item.isActive ? LeafIcon : GitBranchIcon;
    const iconColor = item.isMain
      ? 'text-warning'
      : item.isActive
        ? 'text-success'
        : 'text-primary';
    const iconBg = item.isMain
      ? 'bg-warning/10'
      : item.isActive
        ? 'bg-success/10'
        : 'bg-primary/10';

    return (
      <Pressable
        onPress={() => handleFeaturePress(item)}
        onLongPress={() => deleteFeature(item)}
        className="mb-3">
        <Card
          className={
            isLastUsed
              ? 'border-2 border-primary'
              : item.isMain
                ? 'border-2 border-warning'
                : item.isActive
                  ? 'border-2 border-success'
                  : ''
          }>
          <CardHeader className="flex-row items-center gap-3 pb-3">
            <View
              className={`h-10 w-10 items-center justify-center rounded-full ${iconBg}`}>
              <Icon as={iconAs} className={iconColor} size={20} />
            </View>
            <View className="flex-1">
              <View className="flex-row items-center gap-2">
                <CardTitle className="text-base">{item.name}</CardTitle>
                {item.isMain && (
                  <View className="rounded-full bg-warning px-2 py-0.5">
                    <Text className="text-[10px] font-semibold text-white">
                      Source
                    </Text>
                  </View>
                )}
                {isLastUsed && !item.isMain && (
                  <View className="rounded-full bg-primary px-2 py-0.5">
                    <Text className="text-[10px] font-semibold text-primary-foreground">
                      Dernier
                    </Text>
                  </View>
                )}
                {item.isActive && !isLastUsed && !item.isMain && (
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

  const renderBranchItem = ({ item }: { item: BranchInfo }) => {
    const isSelected = selectedBranch === item.name;

    return (
      <Pressable
        onPress={() => setSelectedBranch(item.name)}
        className={`mb-1.5 flex-row items-center gap-2.5 rounded-lg border px-3 py-2.5 ${
          isSelected ? 'border-primary bg-primary/5' : 'border-border'
        }`}>
        <Icon as={GitBranchIcon} className={isSelected ? 'text-primary' : 'text-muted-foreground'} size={16} />
        <Text className="flex-1 text-sm" numberOfLines={1}>{item.name}</Text>
        <View
          className={`rounded-full px-2 py-0.5 ${
            item.isRemote ? 'bg-blue-500/15' : 'bg-green-500/15'
          }`}>
          <Text
            className={`text-[10px] font-semibold ${
              item.isRemote ? 'text-blue-600' : 'text-green-600'
            }`}>
            {item.isRemote ? item.remoteName || 'remote' : 'local'}
          </Text>
        </View>
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
            <Text>Reessayer</Text>
          </Button>
        </>
      ) : (
        <>
          <View className="mb-4 h-20 w-20 items-center justify-center rounded-full bg-primary/10">
            <Icon as={LeafIcon} className="text-primary" size={40} />
          </View>
          <Text className="mb-2 text-center text-xl font-bold">Aucune feature active</Text>
          <Text className="text-center text-muted-foreground">
            Creez une nouvelle feature pour commencer a developper
          </Text>
        </>
      )}
    </View>
  );

  if (!server) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <Text className="text-destructive">Serveur non trouve</Text>
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
          onPress={openModal}>
          <Icon as={PlusIcon} size={24} className="text-primary-foreground" />
        </Button>

        {/* Branch Modal */}
        <Modal
          visible={showModal}
          transparent
          animationType="fade"
          onRequestClose={closeModal}>
          <KeyboardAvoidingView
            behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
            className="flex-1 justify-end bg-black/70">
            <Card className="w-full" style={{ flex: 1, marginTop: '15%', overflow: 'hidden' }}>
              {/* Header */}
              <CardHeader className="flex-row items-center justify-between pb-2">
                <CardTitle>Ajouter une branche</CardTitle>
                <Pressable onPress={closeModal}>
                  <Icon as={XIcon} className="text-muted-foreground" size={20} />
                </Pressable>
              </CardHeader>

              {/* Tab selector */}
              <View className="mx-4 mb-3 flex-row rounded-lg bg-muted p-1">
                <Pressable
                  onPress={() => { setActiveTab('new'); setSearchQuery(''); setSelectedBranch(null); }}
                  className={`flex-1 items-center rounded-md py-2 ${
                    activeTab === 'new' ? 'bg-background' : ''
                  }`}>
                  <Text
                    className={`text-sm font-medium ${
                      activeTab === 'new' ? 'text-foreground' : 'text-muted-foreground'
                    }`}>
                    Nouvelle branche
                  </Text>
                </Pressable>
                <Pressable
                  onPress={() => { setActiveTab('existing'); setSearchQuery(''); setSelectedBranch(null); }}
                  className={`flex-1 items-center rounded-md py-2 ${
                    activeTab === 'existing' ? 'bg-background' : ''
                  }`}>
                  <Text
                    className={`text-sm font-medium ${
                      activeTab === 'existing' ? 'text-foreground' : 'text-muted-foreground'
                    }`}>
                    Branche existante
                  </Text>
                </Pressable>
              </View>

              {/* Tab content */}
              {activeTab === 'new' ? (
                <View className="flex-1 px-4 pb-4" style={{ overflow: 'hidden' }}>
                  <View className="mb-3 gap-2">
                    <Label nativeID="branchName">Nom de la branche</Label>
                    <Input
                      placeholder="feature/ma-feature"
                      value={branchName}
                      onChangeText={setBranchName}
                      autoCapitalize="none"
                      autoCorrect={false}
                      autoFocus
                      aria-labelledby="branchName"
                    />
                  </View>

                  <Label className="mb-2">Branche source</Label>
                  <Input
                    placeholder="Filtrer..."
                    value={searchQuery}
                    onChangeText={setSearchQuery}
                    autoCapitalize="none"
                    autoCorrect={false}
                    className="mb-2"
                  />

                  {isLoadingBranches ? (
                    <View className="items-center justify-center py-6">
                      <ActivityIndicator size="small" />
                    </View>
                  ) : (
                    <FlatList
                      data={filteredBaseBranches}
                      keyExtractor={(item) => `base-${item.name}-${item.isRemote}`}
                      style={{ flexGrow: 1 }}
                      showsVerticalScrollIndicator={false}
                      renderItem={({ item }) => {
                        const isSelected = baseBranch === item.name;
                        return (
                          <Pressable
                            onPress={() => setBaseBranch(item.name)}
                            className={`mb-1.5 flex-row items-center gap-2.5 rounded-lg border px-3 py-2.5 ${
                              isSelected ? 'border-primary bg-primary/5' : 'border-border'
                            }`}>
                            <Icon as={GitBranchIcon} className={isSelected ? 'text-primary' : 'text-muted-foreground'} size={16} />
                            <Text className="flex-1 text-sm" numberOfLines={1}>{item.name}</Text>
                            <View
                              className={`rounded-full px-2 py-0.5 ${
                                item.isRemote ? 'bg-blue-500/15' : 'bg-green-500/15'
                              }`}>
                              <Text
                                className={`text-[10px] font-semibold ${
                                  item.isRemote ? 'text-blue-600' : 'text-green-600'
                                }`}>
                                {item.isRemote ? item.remoteName || 'remote' : 'local'}
                              </Text>
                            </View>
                          </Pressable>
                        );
                      }}
                    />
                  )}

                  <View className="flex-row gap-3 pt-3">
                    <Button variant="outline" className="flex-1" onPress={closeModal}>
                      <Text>Annuler</Text>
                    </Button>
                    <Button className="flex-1" onPress={createNewBranch} disabled={isCreating}>
                      {isCreating ? (
                        <ActivityIndicator size="small" color="white" />
                      ) : (
                        <Text>Creer</Text>
                      )}
                    </Button>
                  </View>
                </View>
              ) : (
                <View className="flex-1 px-4 pb-4" style={{ overflow: 'hidden' }}>
                  <Input
                    placeholder="Rechercher une branche..."
                    value={searchQuery}
                    onChangeText={setSearchQuery}
                    autoCapitalize="none"
                    autoCorrect={false}
                    className="mb-3"
                  />

                  {isLoadingBranches ? (
                    <View className="items-center justify-center py-8">
                      <ActivityIndicator size="small" />
                      <Text className="mt-2 text-sm text-muted-foreground">
                        Chargement des branches...
                      </Text>
                    </View>
                  ) : filteredBranches.length === 0 ? (
                    <View className="items-center justify-center py-8">
                      <Icon as={GitBranchIcon} className="text-muted-foreground" size={32} />
                      <Text className="mt-2 text-center text-sm text-muted-foreground">
                        {searchQuery
                          ? 'Aucune branche correspondante'
                          : 'Toutes les branches ont deja un worktree'}
                      </Text>
                    </View>
                  ) : (
                    <FlatList
                      data={filteredBranches}
                      keyExtractor={(item) => `${item.name}-${item.isRemote}`}
                      renderItem={renderBranchItem}
                      style={{ flexGrow: 1 }}
                      showsVerticalScrollIndicator={false}
                    />
                  )}

                  <View className="flex-row gap-3 pt-3">
                    <Button variant="outline" className="flex-1" onPress={closeModal}>
                      <Text>Annuler</Text>
                    </Button>
                    <Button
                      className="flex-1"
                      onPress={attachExistingBranch}
                      disabled={isCreating || !selectedBranch}>
                      {isCreating ? (
                        <ActivityIndicator size="small" color="white" />
                      ) : (
                        <>
                          <Icon as={LinkIcon} size={16} className="mr-1 text-primary-foreground" />
                          <Text>Attacher</Text>
                        </>
                      )}
                    </Button>
                  </View>
                </View>
              )}
            </Card>
          </KeyboardAvoidingView>
        </Modal>
      </View>
    </>
  );
}
