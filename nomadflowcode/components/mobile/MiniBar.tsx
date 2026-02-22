import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert,
  Animated,
  Modal,
  PanResponder,
  Pressable,
  ScrollView,
  View,
} from 'react-native';

import { useTranslation } from 'react-i18next';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AgentStatusBadge } from '@/components/shared/AgentStatusBadge';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { cn } from '@/lib/utils';
import { aggregateState } from '@/lib/agent-state';
import type { SessionWithState } from '@/lib/types/session';
import { PlusIcon } from 'lucide-react-native';

interface MiniBarProps {
  sessions: SessionWithState[];
  allSessions: SessionWithState[];
  activeSessionId: string | null;
  worktreeName: string;
  onSwitchSession: (sessionId: string) => void;
  onSwitchWorktree: (worktreeName: string) => void;
  onCreateSession: (agentType: string) => void;
  onDestroySession?: (sessionId: string) => void;
}

export function MiniBar({
  sessions,
  allSessions,
  activeSessionId,
  worktreeName,
  onSwitchSession,
  onSwitchWorktree,
  onCreateSession,
  onDestroySession,
}: MiniBarProps) {
  const { t } = useTranslation();
  const insets = useSafeAreaInsets();

  const AGENT_TYPES = [
    { id: 'shell', label: t('agents.type.shell') },
    { id: 'agent', label: t('agents.type.agent') },
    { id: 'claude', label: t('agents.type.claude_code') },
  ];

  const [worktreeOverlayVisible, setWorktreeOverlayVisible] = useState(false);
  const [createOverlayVisible, setCreateOverlayVisible] = useState(false);
  const scrollViewRef = useRef<ScrollView>(null);
  const slideAnim = useRef(new Animated.Value(0)).current;

  const activeIndex = useMemo(
    () => Math.max(0, sessions.findIndex((s) => s.sessionId === activeSessionId)),
    [sessions, activeSessionId]
  );

  // Refs for PanResponder closures (avoids stale values)
  const activeIndexRef = useRef(activeIndex);
  activeIndexRef.current = activeIndex;
  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;
  const onSwitchSessionRef = useRef(onSwitchSession);
  onSwitchSessionRef.current = onSwitchSession;

  const panResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => false,
        onMoveShouldSetPanResponder: (_, gs) =>
          Math.abs(gs.dx) > 20 && Math.abs(gs.dx) > Math.abs(gs.dy),
        onPanResponderRelease: (_, gs) => {
          if (Math.abs(gs.dx) < 30) return;
          const cur = sessionsRef.current;
          const idx = activeIndexRef.current;
          const dir = gs.dx < 0 ? 1 : -1; // left swipe = next
          const next = Math.max(0, Math.min(cur.length - 1, idx + dir));
          if (next !== idx && cur[next]) {
            Animated.sequence([
              Animated.timing(slideAnim, {
                toValue: dir * -20,
                duration: 100,
                useNativeDriver: true,
              }),
              Animated.timing(slideAnim, {
                toValue: 0,
                duration: 100,
                useNativeDriver: true,
              }),
            ]).start();
            onSwitchSessionRef.current(cur[next].sessionId);
          }
        },
      }),
    [slideAnim]
  );

  // Auto-scroll to active tab when it changes
  useEffect(() => {
    if (scrollViewRef.current && sessions.length > 1) {
      const TAB_WIDTH_ESTIMATE = 100;
      scrollViewRef.current.scrollTo({
        x: Math.max(0, activeIndex * TAB_WIDTH_ESTIMATE - 50),
        animated: true,
      });
    }
  }, [activeIndex, sessions.length]);

  // Worktree groups for overlay (exclude current worktree)
  const worktreeGroups = useMemo(() => {
    const groups = new Map<string, SessionWithState[]>();
    for (const s of allSessions) {
      const list = groups.get(s.worktree) || [];
      list.push(s);
      groups.set(s.worktree, list);
    }
    return Array.from(groups.entries())
      .filter(([name]) => name !== worktreeName)
      .map(([name, sess]) => ({
        name,
        sessions: sess,
        aggregatedState: aggregateState(sess),
      }));
  }, [allSessions, worktreeName]);

  return (
    <>
      <View
        className="border-t border-border bg-card"
        style={{ height: 56 }}
        {...(sessions.length > 0 ? panResponder.panHandlers : {})}>
        <View className="flex-1 flex-row items-center px-4">
          {/* Worktree label — long-press to switch */}
          <Pressable
            onLongPress={() => setWorktreeOverlayVisible(true)}
            delayLongPress={400}
            style={{ minHeight: 44 }}
            className="mr-2 max-w-[80px] justify-center">
            <Text className="text-xs text-muted-foreground" numberOfLines={1}>
              {worktreeName}
            </Text>
          </Pressable>

          {sessions.length === 0 ? (
            <Text className="flex-1 text-xs text-muted-foreground">{t('agents.none')}</Text>
          ) : (
            /* Tab pills with slide animation */
            <Animated.View className="flex-1" style={{ transform: [{ translateX: slideAnim }] }}>
              <ScrollView
                ref={scrollViewRef}
                horizontal
                scrollEnabled={false}
                showsHorizontalScrollIndicator={false}
                contentContainerStyle={{ alignItems: 'center', gap: 4, paddingRight: 8 }}>
                {sessions.map((session) => {
                  const isActive = session.sessionId === activeSessionId;
                  const label = `${session.agentType}-${session.agentNumber}`;
                  return (
                    <Pressable
                      key={session.sessionId}
                      onPress={() => onSwitchSession(session.sessionId)}
                      onLongPress={() => {
                        if (!onDestroySession) return;
                        Alert.alert(
                          t('agents.close.confirm_title'),
                          t('agents.close.confirm_message'),
                          [
                            { text: t('agents.close.cancel'), style: 'cancel' },
                            {
                              text: t('agents.close.confirm'),
                              style: 'destructive',
                              onPress: () => onDestroySession(session.sessionId),
                            },
                          ]
                        );
                      }}
                      delayLongPress={500}
                      style={{ minHeight: 44 }}
                      className={cn(
                        'flex-row items-center gap-1 rounded-lg px-3',
                        isActive ? 'bg-accent' : 'bg-transparent'
                      )}>
                      <AgentStatusBadge state={session.agentState} size="sm" />
                      <Text
                        className={cn(
                          'text-xs font-medium',
                          isActive ? 'text-accent-foreground' : 'text-muted-foreground'
                        )}
                        numberOfLines={1}>
                        {label}
                      </Text>
                    </Pressable>
                  );
                })}
              </ScrollView>
            </Animated.View>
          )}

          {/* Create session button */}
          <Pressable
            onPress={() => setCreateOverlayVisible(true)}
            style={{ minWidth: 44, minHeight: 44 }}
            className="items-center justify-center">
            <View className="h-7 w-7 items-center justify-center rounded-full bg-muted">
              <Icon as={PlusIcon} className="text-muted-foreground" size={14} />
            </View>
          </Pressable>
        </View>
      </View>

      {/* Worktree Switcher Overlay */}
      <Modal
        visible={worktreeOverlayVisible}
        transparent
        animationType="fade"
        onRequestClose={() => setWorktreeOverlayVisible(false)}>
        <View className="flex-1 justify-end bg-black/50">
          <Pressable className="flex-1" onPress={() => setWorktreeOverlayVisible(false)} />
          <View className="rounded-t-2xl bg-card p-4"
            style={{ paddingBottom: Math.max(insets.bottom, 16) + 16 }}>
            <Text className="mb-3 text-sm font-semibold">{t('worktrees.switch.title')}</Text>

            {/* Current worktree */}
            <View className="mb-2 flex-row items-center gap-3 rounded-lg bg-accent px-4 py-3">
              <AgentStatusBadge state={aggregateState(sessions)} size="sm" />
              <Text className="flex-1 text-sm font-medium text-accent-foreground">
                {worktreeName}
              </Text>
              <Text className="text-xs text-muted-foreground">{t('worktrees.badge.current')}</Text>
            </View>

            {worktreeGroups.length === 0 ? (
              <Text className="py-4 text-center text-sm text-muted-foreground">
                {t('worktrees.switch.no_other')}
              </Text>
            ) : (
              worktreeGroups.map((group) => (
                <Pressable
                  key={group.name}
                  onPress={() => {
                    setWorktreeOverlayVisible(false);
                    onSwitchWorktree(group.name);
                  }}
                  className="flex-row items-center gap-3 rounded-lg px-4 py-3 active:bg-muted">
                  <AgentStatusBadge state={group.aggregatedState} size="sm" />
                  <Text className="flex-1 text-sm font-medium">{group.name}</Text>
                  <Text className="text-xs text-muted-foreground">
                    {t('agents.count', { count: group.sessions.length })}
                  </Text>
                </Pressable>
              ))
            )}
          </View>
        </View>
      </Modal>

      {/* New Agent Overlay */}
      <Modal
        visible={createOverlayVisible}
        transparent
        animationType="fade"
        onRequestClose={() => setCreateOverlayVisible(false)}>
        <View className="flex-1 justify-end bg-black/50">
          <Pressable className="flex-1" onPress={() => setCreateOverlayVisible(false)} />
          <View className="rounded-t-2xl bg-card p-4"
            style={{ paddingBottom: Math.max(insets.bottom, 16) + 16 }}>
            <Text className="mb-3 text-sm font-semibold">{t('agents.create.title')}</Text>
            {AGENT_TYPES.map((type) => (
              <Pressable
                key={type.id}
                onPress={() => {
                  setCreateOverlayVisible(false);
                  onCreateSession(type.id);
                }}
                className="flex-row items-center gap-3 rounded-lg px-4 py-3 active:bg-muted">
                <Text className="flex-1 text-sm font-medium">{type.label}</Text>
              </Pressable>
            ))}
          </View>
        </View>
      </Modal>
    </>
  );
}
