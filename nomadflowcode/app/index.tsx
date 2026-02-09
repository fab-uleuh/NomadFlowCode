import { Button } from '@/components/ui/button';
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { useStorage } from '@/lib/context/storage-context';
import type { Server } from '@shared';
import { Link, Stack, useRouter } from 'expo-router';
import {
  MonitorIcon,
  PlusIcon,
  SettingsIcon,
  ChevronRightIcon,
  RocketIcon,
  PencilIcon,
} from 'lucide-react-native';
import { useColorScheme } from 'nativewind';
import * as React from 'react';
import { FlatList, Pressable, View, Alert } from 'react-native';

export default function ServersScreen() {
  const router = useRouter();
  const { colorScheme } = useColorScheme();
  const { servers, deleteServer, lastSelection, isLoading } = useStorage();

  const formatLastConnected = (timestamp?: number) => {
    if (!timestamp) return 'Jamais connecté';
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffMins < 1) return "À l'instant";
    if (diffMins < 60) return `Il y a ${diffMins} min`;
    if (diffHours < 24) return `Il y a ${diffHours}h`;
    if (diffDays < 7) return `Il y a ${diffDays}j`;
    return date.toLocaleDateString('fr-FR');
  };

  const handleServerPress = (server: Server) => {
    router.push({ pathname: '/repos', params: { serverId: server.id } });
  };

  const handleEditServer = (server: Server) => {
    router.push({ pathname: '/add-server', params: { serverId: server.id } });
  };

  const handleServerLongPress = (server: Server) => {
    Alert.alert(server.name, 'Que voulez-vous faire ?', [
      {
        text: 'Modifier',
        onPress: () => handleEditServer(server),
      },
      {
        text: 'Supprimer',
        style: 'destructive',
        onPress: () => confirmDelete(server),
      },
      { text: 'Annuler', style: 'cancel' },
    ]);
  };

  const confirmDelete = (server: Server) => {
    Alert.alert('Supprimer le serveur ?', `Êtes-vous sûr de vouloir supprimer "${server.name}" ?`, [
      { text: 'Annuler', style: 'cancel' },
      {
        text: 'Supprimer',
        style: 'destructive',
        onPress: () => deleteServer(server.id),
      },
    ]);
  };

  const renderServer = ({ item }: { item: Server }) => {
    const isLastUsed = lastSelection.serverId === item.id;

    return (
      <Pressable
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
                      Dernier
                    </Text>
                  </View>
                )}
              </View>
              <CardDescription className="text-xs">{item.ttydUrl}</CardDescription>
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
      <Text className="mb-2 text-center text-xl font-bold">Aucun serveur configuré</Text>
      <Text className="mb-6 text-center text-muted-foreground">
        Ajoutez votre premier serveur pour commencer à coder depuis n'importe où
      </Text>
      <Link href="/add-server" asChild>
        <Button>
          <Icon as={PlusIcon} className="mr-2" size={18} />
          <Text>Ajouter un serveur</Text>
        </Button>
      </Link>
    </View>
  );

  return (
    <>
      <Stack.Screen
        options={{
          title: 'Serveurs',
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
        <FlatList
          data={servers}
          keyExtractor={(item) => item.id}
          renderItem={renderServer}
          ListEmptyComponent={renderEmpty}
          contentContainerStyle={{
            padding: 16,
            flexGrow: servers.length === 0 ? 1 : undefined,
          }}
          contentInsetAdjustmentBehavior="automatic"
          showsVerticalScrollIndicator={false}
        />

        {servers.length > 0 && (
          <Link href="/add-server" asChild>
            <Button
              size="icon"
              className="absolute bottom-6 right-6 h-14 w-14 rounded-full shadow-lg bg-primary active:bg-primary/90">
              <Icon as={PlusIcon} size={24} className="text-white" />
            </Button>
          </Link>
        )}
      </View>
    </>
  );
}
