import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, Pressable, View } from 'react-native';
import { CircleCheck } from 'lucide-react-native';

import { AgentStatusBadge } from '@/components/shared/AgentStatusBadge';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { cn } from '@/lib/utils';
import { destroyPane, executeServerCommand, fetchPanes } from '@/lib/server-commands';
import type { Pane } from '@/lib/types/terminal-messages';
import type { SessionWithState } from '@/lib/types/session';
import type { Server, Repository } from '@shared';

interface AgentItem {
  serverId: string;
  serverName: string;
  server: Server;
  session: SessionWithState;
  paneId: number;
  repoPath: string | undefined;
}

interface AgentsWaitingSectionProps {
  servers: Server[];
  onAgentPress: (params: { serverId: string; repoPath: string; featureName: string }) => void;
}

/** Polls /api/list-panes for a single server and reports results upward. */
function ServerPanePoller({
  server,
  onData,
}: {
  server: Server;
  onData: (serverId: string, panes: Pane[], repoMap: Map<string, string>) => void;
}) {
  const [repoMap, setRepoMap] = useState<Map<string, string>>(new Map());
  const prevDataKeyRef = useRef('');

  // Fetch repo map once
  useEffect(() => {
    let cancelled = false;
    async function fetchRepos() {
      const result = await executeServerCommand(server, { action: 'list-repos' });
      if (!cancelled && result.success && result.data?.repos) {
        const map = new Map<string, string>();
        for (const repo of result.data.repos as Repository[]) {
          map.set(repo.name, repo.path);
        }
        setRepoMap(map);
      }
    }
    if (server.apiUrl && server.authToken) {
      fetchRepos();
    }
    return () => { cancelled = true; };
  }, [server.apiUrl, server.authToken, server.id]);

  // Poll panes every 3 seconds
  const [panes, setPanes] = useState<Pane[]>([]);
  useEffect(() => {
    let cancelled = false;
    async function poll() {
      const result = await fetchPanes(server);
      if (!cancelled && result.success && result.data?.panes) {
        setPanes(result.data.panes);
      }
    }
    poll();
    const interval = setInterval(poll, 3000);
    return () => { cancelled = true; clearInterval(interval); };
  }, [server.apiUrl, server.authToken, server.id]);

  // Notify parent when panes or repoMap change
  useEffect(() => {
    const paneKey = panes.map((p) => `${p.id}:${p.label}:${p.agentState}`).join('|');
    const repoKey = Array.from(repoMap.entries()).map(([k, v]) => `${k}=${v}`).join('|');
    const dataKey = `${paneKey}::${repoKey}`;
    if (dataKey === prevDataKeyRef.current) return;
    prevDataKeyRef.current = dataKey;
    onData(server.id, panes, repoMap);
  }, [panes, repoMap, server.id, onData]);

  return null;
}

export function AgentsWaitingSection({ servers, onAgentPress }: AgentsWaitingSectionProps) {
  const { t } = useTranslation();
  const [serverDataMap, setServerDataMap] = useState<
    Map<string, { panes: Pane[]; repoMap: Map<string, string> }>
  >(new Map());
  const [initialLoading, setInitialLoading] = useState(true);

  const connectedServers = useMemo(
    () => servers.filter((s) => s.apiUrl && s.authToken),
    [servers]
  );

  const handleServerData = useCallback(
    (serverId: string, panes: Pane[], repoMap: Map<string, string>) => {
      setServerDataMap((prev) => {
        const next = new Map(prev);
        next.set(serverId, { panes, repoMap });
        return next;
      });
      setInitialLoading(false);
    },
    []
  );

  const allAgents = useMemo(() => {
    const agents: AgentItem[] = [];
    for (const server of connectedServers) {
      const data = serverDataMap.get(server.id);
      if (!data) continue;
      for (const pane of data.panes) {
        agents.push({
          serverId: server.id,
          serverName: server.name,
          server,
          paneId: pane.id,
          session: {
            sessionId: String(pane.id),
            windowName: pane.label,
            repo: pane.repo,
            worktree: pane.worktree,
            agentType: pane.agentType,
            agentNumber: pane.agentNumber,
            agentState: pane.agentState,
            stateTimestamp: null,
          },
          repoPath: data.repoMap.get(pane.repo),
        });
      }
    }
    return agents;
  }, [connectedServers, serverDataMap]);

  const handleAgentPress = (agent: AgentItem) => {
    if (!agent.repoPath) return;
    onAgentPress({
      serverId: agent.serverId,
      repoPath: agent.repoPath,
      featureName: agent.session.worktree,
    });
  };

  const handleAgentLongPress = (agent: AgentItem) => {
    Alert.alert(t('agents.close.confirm_title'), t('agents.close.confirm_message'), [
      { text: t('agents.close.cancel'), style: 'cancel' },
      {
        text: t('agents.close.confirm'),
        style: 'destructive',
        onPress: async () => {
          const result = await destroyPane(agent.server, agent.paneId);
          if (result.success) {
            // Optimistically remove the destroyed pane from local state
            setServerDataMap((prev) => {
              const next = new Map(prev);
              const data = next.get(agent.serverId);
              if (data) {
                next.set(agent.serverId, {
                  ...data,
                  panes: data.panes.filter((p) => p.id !== agent.paneId),
                });
              }
              return next;
            });
          } else {
            Alert.alert(t('common.error'), result.error || t('common.error.unknown'));
          }
        },
      },
    ]);
  };

  if (connectedServers.length === 0) return null;

  return (
    <View className="mb-4">
      <Text className="mb-2 text-sm font-semibold text-foreground">{t('agents.waiting.title')}</Text>

      {connectedServers.map((server) => (
        <ServerPanePoller
          key={server.id}
          server={server}
          onData={handleServerData}
        />
      ))}

      {initialLoading ? (
        <View className="gap-2">
          <SkeletonRow />
          <SkeletonRow />
        </View>
      ) : allAgents.length === 0 ? (
        <View className="flex-row items-center gap-2 rounded-lg bg-muted/50 px-4 py-3">
          <Icon as={CircleCheck} size={18} className="text-success" />
          <Text className="text-sm text-muted-foreground">
            {t('agents.waiting.all_clear')}
          </Text>
        </View>
      ) : (
        <View className="gap-2">
          {allAgents.map((agent) => (
            <Pressable
              key={`${agent.serverId}-${agent.paneId}`}
              onPress={() => handleAgentPress(agent)}
              onLongPress={() => handleAgentLongPress(agent)}
              disabled={!agent.repoPath}
              className={cn(
                'flex-row items-center gap-3 rounded-lg bg-card px-4 py-3 active:bg-muted',
                !agent.repoPath && 'opacity-50'
              )}>
              <AgentStatusBadge state={agent.session.agentState} size="md" />
              <View className="flex-1">
                <Text className="text-sm font-medium text-foreground" numberOfLines={1}>
                  {agent.session.worktree}
                  {agent.session.agentNumber > 0
                    ? ` > ${agent.session.agentType}-${agent.session.agentNumber}`
                    : ''}
                </Text>
                <Text className="text-xs text-muted-foreground" numberOfLines={1}>
                  {agent.serverName} &middot; {agent.session.repo}
                </Text>
              </View>
            </Pressable>
          ))}
        </View>
      )}
    </View>
  );
}

function SkeletonRow() {
  return (
    <View className="flex-row items-center gap-3 rounded-lg bg-card px-4 py-3">
      <View className="h-3 w-3 rounded-full bg-muted animate-pulse" />
      <View className="flex-1 gap-1.5">
        <View className="h-3.5 w-3/4 rounded bg-muted animate-pulse" />
        <View className="h-3 w-1/2 rounded bg-muted animate-pulse" />
      </View>
      <View className="h-3 w-8 rounded bg-muted animate-pulse" />
    </View>
  );
}
