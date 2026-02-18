import { AgentsWaitingSection } from '@/components/mobile/AgentsWaitingSection';
import { ResumeButton } from '@/components/mobile/ResumeButton';
import { Button } from '@/components/ui/button';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import { fetchSessions } from '@/lib/server-commands';
import type { AgentStateKind } from '@/lib/types/session';
import type { Server } from '@shared';
import Constants from 'expo-constants';
import { Link, Stack, useRouter } from 'expo-router';
import {
  MonitorIcon,
  PlusIcon,
  SettingsIcon,
  ChevronRightIcon,
  RocketIcon,
  PencilIcon,
} from 'lucide-react-native';
import * as React from 'react';
import { useTranslation } from 'react-i18next';
import { Pressable, ScrollView, View, Alert } from 'react-native';

export default function ServersScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const { servers, deleteServer, lastSelection, getServer } = useStorage();

  const lastServer = lastSelection.serverId ? getServer(lastSelection.serverId) : undefined;

  const [resumeAgentInfo, setResumeAgentInfo] = React.useState<{
    state: AgentStateKind;
    name: string;
  } | null>(null);

  React.useEffect(() => {
    if (!lastServer?.apiUrl || !lastServer?.authToken || !lastSelection.featureName) {
      setResumeAgentInfo(null);
      return;
    }
    async function loadResumeInfo() {
      const result = await fetchSessions(lastServer!);
      if (result.success && result.data?.sessions) {
        const match = result.data.sessions.find(
          (s: { worktree: string }) => s.worktree === lastSelection.featureName
        );
        if (match) {
          setResumeAgentInfo({
            state: match.agentState,
            name: `${match.agentType}-${match.agentNumber}`,
          });
        } else {
          setResumeAgentInfo(null);
        }
      }
    }
    loadResumeInfo();
  }, [lastServer, lastSelection.featureName]);

  const formatLastConnected = (timestamp?: number) => {
    if (!timestamp) return t('servers.last_connected.never');
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffMins < 1) return t('servers.last_connected.just_now');
    if (diffMins < 60) return t('servers.last_connected.minutes_ago', { minutes: diffMins });
    if (diffHours < 24) return t('servers.last_connected.hours_ago', { hours: diffHours });
    if (diffDays < 7) return t('servers.last_connected.days_ago', { days: diffDays });
    return date.toLocaleDateString();
  };

  const handleServerPress = (server: Server) => {
    router.push({ pathname: '/repos', params: { serverId: server.id } });
  };

  const handleEditServer = (server: Server) => {
    router.push({ pathname: '/add-server', params: { serverId: server.id } });
  };

  const handleServerLongPress = (server: Server) => {
    Alert.alert(server.name, t('common.what_to_do'), [
      {
        text: t('common.edit'),
        onPress: () => handleEditServer(server),
      },
      {
        text: t('common.delete'),
        style: 'destructive',
        onPress: () => confirmDelete(server),
      },
      { text: t('common.cancel'), style: 'cancel' },
    ]);
  };

  const confirmDelete = (server: Server) => {
    Alert.alert(t('servers.delete.confirm_title'), t('servers.delete.confirm_message', { name: server.name }), [
      { text: t('common.cancel'), style: 'cancel' },
      {
        text: t('common.delete'),
        style: 'destructive',
        onPress: () => deleteServer(server.id),
      },
    ]);
  };

  const handleResumePress = () => {
    if (lastSelection.serverId && lastSelection.repoPath && lastSelection.featureName) {
      router.push({
        pathname: '/terminal',
        params: {
          serverId: lastSelection.serverId,
          repoPath: lastSelection.repoPath,
          featureName: lastSelection.featureName,
        },
      });
    }
  };

  const handleAgentPress = (params: { serverId: string; repoPath: string; featureName: string }) => {
    router.push({
      pathname: '/terminal',
      params,
    });
  };

  const renderServerCard = (item: Server) => {
    const isLastUsed = lastSelection.serverId === item.id;

    return (
      <Pressable
        key={item.id}
        onPress={() => handleServerPress(item)}
        onLongPress={() => handleServerLongPress(item)}
        className="mb-3">
        <Card className={isLastUsed ? 'border-2 border-primary' : ''}>
          <CardHeader className="flex-row items-center gap-3 pb-3">
            <View className="h-10 w-10 items-center justify-center rounded-full bg-primary/10">
              <Icon as={MonitorIcon} className="text-primary" size={20} />
            </View>
            <View className="flex-1">
              <View className="flex-row items-center gap-2">
                <CardTitle className="text-base">{item.name}</CardTitle>
                {isLastUsed && (
                  <View className="rounded-full bg-primary px-2 py-0.5">
                    <Text className="text-[10px] font-semibold text-primary-foreground">
                      {t('servers.badge.last_used')}
                    </Text>
                  </View>
                )}
              </View>
              <CardDescription className="text-xs">{item.apiUrl}</CardDescription>
              <CardDescription className="text-xs">
                {formatLastConnected(item.lastConnected)}
              </CardDescription>
            </View>
            <Pressable
              onPress={(e) => {
                e.stopPropagation();
                handleEditServer(item);
              }}
              hitSlop={8}
              className="mr-1 p-2 rounded-full active:bg-muted">
              <Icon as={PencilIcon} className="text-muted-foreground" size={18} />
            </Pressable>
            <Icon as={ChevronRightIcon} className="text-muted-foreground" size={20} />
          </CardHeader>
        </Card>
      </Pressable>
    );
  };

  const renderEmpty = () => (
    <View className="flex-1 items-center justify-center p-8">
      <View className="mb-4 h-20 w-20 items-center justify-center rounded-full bg-primary/10">
        <Icon as={RocketIcon} className="text-primary" size={40} />
      </View>
      <Text className="mb-2 text-center text-xl font-bold">{t('servers.empty.title')}</Text>
      <Text className="mb-6 text-center text-muted-foreground">
        {t('servers.empty.description')}
      </Text>
      <Link href="/add-server" asChild>
        <Button>
          <Icon as={PlusIcon} className="mr-2" size={18} />
          <Text>{t('servers.add.button')}</Text>
        </Button>
      </Link>
    </View>
  );

  return (
    <>
      <Stack.Screen
        options={{
          title: t('servers.title'),
          headerRight: () => (
            <Link href="/settings" asChild>
              <Button variant="ghost" size="icon" className="mr-2">
                <Icon as={SettingsIcon} size={22} />
              </Button>
            </Link>
          ),
        }}
      />
      <View className="flex-1 bg-background">
        {servers.length === 0 ? (
          renderEmpty()
        ) : (
          <ScrollView
            contentContainerStyle={{ padding: 16 }}
            contentInsetAdjustmentBehavior="automatic"
            showsVerticalScrollIndicator={false}>
            <ResumeButton
              server={lastServer}
              lastSelection={lastSelection}
              agentState={resumeAgentInfo?.state}
              agentName={resumeAgentInfo?.name}
              onPress={handleResumePress}
            />

            <AgentsWaitingSection servers={servers} onAgentPress={handleAgentPress} />

            <Text className="mb-2 text-sm font-semibold text-foreground">{t('servers.title')}</Text>
            {servers.map(renderServerCard)}
          </ScrollView>
        )}

        {servers.length > 0 && (
          <Link href="/add-server" asChild>
            <Button
              size="icon"
              className="absolute bottom-6 right-6 h-14 w-14 rounded-full shadow-lg bg-primary active:bg-primary/90">
              <Icon as={PlusIcon} size={24} className="text-white" />
            </Button>
          </Link>
        )}

        <Text className="absolute bottom-4 left-4 text-xs text-muted-foreground/50">
          v{Constants.expoConfig?.version ?? ''}
        </Text>
      </View>
    </>
  );
}
