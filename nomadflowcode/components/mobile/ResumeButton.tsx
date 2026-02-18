import { LinearGradient } from 'expo-linear-gradient';
import { Play } from 'lucide-react-native';
import { useTranslation } from 'react-i18next';
import { Pressable, View } from 'react-native';

import { AgentStatusBadge } from '@/components/shared/AgentStatusBadge';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import type { AgentStateKind } from '@/lib/types/session';
import type { Server } from '@shared';

interface LastSelection {
  serverId?: string;
  repoPath?: string;
  featureName?: string;
}

interface ResumeButtonProps {
  server: Server | undefined;
  lastSelection: LastSelection;
  agentState?: AgentStateKind;
  agentName?: string;
  onPress: () => void;
}

export function ResumeButton({
  server,
  lastSelection,
  agentState,
  agentName,
  onPress,
}: ResumeButtonProps) {
  const { t } = useTranslation();
  if (!server || !lastSelection.serverId || !lastSelection.featureName) {
    return null;
  }

  const worktreeLabel = lastSelection.featureName;
  const agentLabel = agentName || 'terminal';

  return (
    <Pressable onPress={onPress} className="mb-4">
      <LinearGradient
        colors={['#5336E2', '#8B5CF6']}
        start={{ x: 0, y: 0 }}
        end={{ x: 1, y: 0 }}
        style={{ borderRadius: 12, minHeight: 56 }}>
        <View className="flex-row items-center gap-3 px-4 py-3.5">
          <View className="h-8 w-8 items-center justify-center rounded-full bg-white/20">
            <Icon as={Play} size={16} className="text-white" />
          </View>
          <View className="flex-1">
            <Text className="text-xs font-medium text-white/70">{t('common.resume')}</Text>
            <Text className="text-sm font-semibold text-white" numberOfLines={1}>
              {worktreeLabel} &gt; {agentLabel}
            </Text>
          </View>
          {agentState && <AgentStatusBadge state={agentState} size="sm" className="opacity-90" />}
        </View>
      </LinearGradient>
    </Pressable>
  );
}
