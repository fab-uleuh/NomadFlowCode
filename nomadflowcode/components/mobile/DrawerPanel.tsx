import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Animated,
  Easing,
  PanResponder,
  Pressable,
  ScrollView,
  View,
  useWindowDimensions,
} from 'react-native';

import { useTranslation } from 'react-i18next';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AgentStatusBadge } from '@/components/shared/AgentStatusBadge';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { cn } from '@/lib/utils';
import { aggregateState } from '@/lib/agent-state';
import { fetchWorktreeStatus } from '@/lib/server-commands';
import type { SessionWithState } from '@/lib/types/session';
import type { Feature, FileChange, Server } from '@shared';
import { CheckCircleIcon, RefreshCwIcon, XIcon } from 'lucide-react-native';

// --- DrawerPanel (reusable animated slide-over drawer) ---

interface DrawerPanelProps {
  side: 'left' | 'right';
  isOpen: boolean;
  onClose: () => void;
  children: React.ReactNode;
}

export function DrawerPanel({ side, isOpen, onClose, children }: DrawerPanelProps) {
  const insets = useSafeAreaInsets();
  const { width: screenWidth } = useWindowDimensions();
  const drawerWidth = Math.min(screenWidth * 0.8, 320);
  const closedX = side === 'left' ? -drawerWidth : drawerWidth;
  const translateX = useRef(new Animated.Value(closedX)).current;
  const backdropOpacity = useRef(new Animated.Value(0)).current;
  const isOpenRef = useRef(isOpen);
  isOpenRef.current = isOpen;

  // Sync translateX when closedX changes (e.g. rotation) while drawer is closed
  useEffect(() => {
    if (!isOpen) {
      translateX.setValue(closedX);
    }
  }, [closedX, isOpen, translateX]);

  useEffect(() => {
    if (isOpen) {
      Animated.parallel([
        Animated.timing(translateX, {
          toValue: 0,
          duration: 250,
          easing: Easing.out(Easing.ease),
          useNativeDriver: true,
        }),
        Animated.timing(backdropOpacity, {
          toValue: 1,
          duration: 250,
          useNativeDriver: true,
        }),
      ]).start();
    } else {
      Animated.parallel([
        Animated.timing(translateX, {
          toValue: closedX,
          duration: 200,
          easing: Easing.in(Easing.ease),
          useNativeDriver: true,
        }),
        Animated.timing(backdropOpacity, {
          toValue: 0,
          duration: 200,
          useNativeDriver: true,
        }),
      ]).start();
    }
  }, [isOpen, closedX, translateX, backdropOpacity]);

  // Swipe-to-close PanResponder (left drawer: swipe left, right drawer: swipe right)
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  const panResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => false,
        onMoveShouldSetPanResponder: (_, gs) =>
          Math.abs(gs.dx) > 20 && Math.abs(gs.dx) > Math.abs(gs.dy),
        onPanResponderRelease: (_, gs) => {
          if (!isOpenRef.current) return;
          const shouldClose =
            (side === 'left' && gs.dx < -30) || (side === 'right' && gs.dx > 30);
          if (shouldClose) onCloseRef.current();
        },
      }),
    [side]
  );

  return (
    <View
      style={{ position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, zIndex: 200 }}
      pointerEvents={isOpen ? 'auto' : 'none'}>
      {/* Backdrop */}
      <Animated.View
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          backgroundColor: 'rgba(0,0,0,0.5)',
          opacity: backdropOpacity,
        }}>
        <Pressable style={{ flex: 1 }} onPress={onClose} />
      </Animated.View>

      {/* Drawer panel */}
      <Animated.View
        {...panResponder.panHandlers}
        style={{
          position: 'absolute',
          top: 0,
          bottom: 0,
          width: drawerWidth,
          paddingTop: insets.top,
          ...(side === 'left' ? { left: 0 } : { right: 0 }),
          transform: [{ translateX }],
        }}
        className={cn('bg-card', side === 'left' ? 'rounded-r-2xl' : 'rounded-l-2xl')}>
        {children}
      </Animated.View>
    </View>
  );
}

// --- WorktreeDrawerContent ---

interface WorktreeDrawerContentProps {
  allSessions: SessionWithState[];
  activeSessionId: string | null;
  currentWorktree: string;
  /** All worktrees from the features API (shows worktrees even without agents) */
  features?: Feature[];
  onSwitchSession: (sessionId: string) => void;
  onSwitchWorktree: (worktreeName: string) => void;
  onClose: () => void;
}

export function WorktreeDrawerContent({
  allSessions,
  activeSessionId,
  currentWorktree,
  features = [],
  onSwitchSession,
  onSwitchWorktree,
  onClose,
}: WorktreeDrawerContentProps) {
  const { t } = useTranslation();
  const worktreeGroups = useMemo(() => {
    // Phase 1: group agent sessions by their worktree name
    const groups = new Map<string, SessionWithState[]>();
    for (const s of allSessions) {
      const list = groups.get(s.worktree) || [];
      list.push(s);
      groups.set(s.worktree, list);
    }
    // Phase 2: add worktrees that have no agent sessions (from features API)
    for (const f of features) {
      if (!groups.has(f.name)) {
        groups.set(f.name, []);
      }
    }
    const result: { name: string; sessions: SessionWithState[]; isCurrent: boolean }[] = [];
    for (const [name, sess] of groups.entries()) {
      result.push({ name, sessions: sess, isCurrent: name === currentWorktree });
    }
    result.sort((a, b) => (a.isCurrent === b.isCurrent ? 0 : a.isCurrent ? -1 : 1));
    return result;
  }, [allSessions, features, currentWorktree]);

  return (
    <View className="flex-1">
      {/* Header */}
      <View className="flex-row items-center justify-between border-b border-border px-4 py-3">
        <Text className="text-base font-semibold">{t('worktrees.title')}</Text>
        <Pressable
          onPress={onClose}
          style={{ minWidth: 44, minHeight: 44 }}
          className="items-center justify-center">
          <Icon as={XIcon} className="text-muted-foreground" size={20} />
        </Pressable>
      </View>

      {/* Worktree list */}
      <ScrollView className="flex-1" contentContainerStyle={{ paddingBottom: 24 }} nestedScrollEnabled={true}>
        {worktreeGroups.map((group) => (
          <View key={group.name} className="mt-2">
            {/* Worktree header */}
            <Pressable
              onPress={() => {
                if (!group.isCurrent) {
                  onSwitchWorktree(group.name);
                  onClose();
                }
              }}
              className={cn(
                'flex-row items-center gap-2 px-4 py-2',
                group.isCurrent && 'bg-accent/50'
              )}>
              <AgentStatusBadge state={aggregateState(group.sessions)} size="md" />
              <Text
                className={cn(
                  'flex-1 text-sm font-medium',
                  group.isCurrent && 'text-accent-foreground'
                )}
                numberOfLines={1}>
                {group.name}
              </Text>
              {group.isCurrent && (
                <Text className="text-xs text-muted-foreground">{t('worktrees.badge.current')}</Text>
              )}
            </Pressable>

            {/* Sessions under this worktree */}
            {group.sessions.map((session) => {
              const isActive = session.sessionId === activeSessionId;
              const label = `${session.agentType}-${session.agentNumber}`;
              return (
                <Pressable
                  key={session.sessionId}
                  onPress={() => {
                    onSwitchSession(session.sessionId);
                    onClose();
                  }}
                  className={cn(
                    'flex-row items-center gap-2 py-2 pl-8 pr-4',
                    isActive && 'bg-accent'
                  )}>
                  <AgentStatusBadge state={session.agentState} size="sm" />
                  <Text
                    className={cn(
                      'text-sm',
                      isActive ? 'font-medium text-accent-foreground' : 'text-foreground'
                    )}
                    numberOfLines={1}>
                    {label}
                  </Text>
                </Pressable>
              );
            })}
          </View>
        ))}

        {worktreeGroups.length === 0 && (
          <Text className="px-4 py-8 text-center text-sm text-muted-foreground">
            {t('worktrees.empty')}
          </Text>
        )}
      </ScrollView>
    </View>
  );
}

// --- DiffDrawerContent ---

const STATUS_ICONS: Record<string, { label: string; color: string }> = {
  modified: { label: 'M', color: 'text-yellow-500' },
  new: { label: 'A', color: 'text-green-500' },
  deleted: { label: 'D', color: 'text-red-500' },
  renamed: { label: 'R', color: 'text-blue-500' },
  conflicted: { label: 'C', color: 'text-red-500' },
};

interface DiffDrawerContentProps {
  server: Server;
  worktreePath: string;
  isOpen: boolean;
  onClose: () => void;
  onFileClick?: (filePath: string) => void;
}

export function DiffDrawerContent({
  server,
  worktreePath,
  isOpen,
  onClose,
  onFileClick,
}: DiffDrawerContentProps) {
  const { t } = useTranslation();
  const [files, setFiles] = useState<FileChange[]>([]);
  const [summary, setSummary] = useState<{
    totalFiles: number;
    additions: number;
    deletions: number;
  } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const prevOpenRef = useRef(false);

  const fetchStatus = useCallback(async () => {
    if (!worktreePath) return;
    setLoading(true);
    setError(null);
    const result = await fetchWorktreeStatus(server, worktreePath);
    setLoading(false);
    if (result.success && result.data) {
      setFiles(result.data.files);
      setSummary({
        totalFiles: result.data.files.length,
        additions: result.data.summary.totalAdditions,
        deletions: result.data.summary.totalDeletions,
      });
    } else {
      setError(result.error || t('diff.error.fetch_status'));
    }
  }, [server, worktreePath]);

  // Lazy-load: fetch when drawer opens or worktreePath changes while open
  const prevWorktreeRef = useRef(worktreePath);
  useEffect(() => {
    const justOpened = isOpen && !prevOpenRef.current;
    const worktreeChanged = isOpen && worktreePath !== prevWorktreeRef.current;
    if (justOpened || worktreeChanged) {
      fetchStatus();
    }
    prevOpenRef.current = isOpen;
    prevWorktreeRef.current = worktreePath;
  }, [isOpen, worktreePath, fetchStatus]);

  return (
    <View className="flex-1">
      {/* Header */}
      <View className="flex-row items-center justify-between border-b border-border px-4 py-3">
        <Text className="text-base font-semibold">{t('diff.title')}</Text>
        <View className="flex-row items-center gap-1">
          <Pressable
            onPress={fetchStatus}
            style={{ minWidth: 44, minHeight: 44 }}
            className="items-center justify-center">
            <Icon as={RefreshCwIcon} className="text-muted-foreground" size={18} />
          </Pressable>
          <Pressable
            onPress={onClose}
            style={{ minWidth: 44, minHeight: 44 }}
            className="items-center justify-center">
            <Icon as={XIcon} className="text-muted-foreground" size={20} />
          </Pressable>
        </View>
      </View>

      {/* Summary */}
      {summary && !loading && files.length > 0 && (
        <View className="border-b border-border px-4 py-2">
          <Text className="text-xs text-muted-foreground">
            {t('diff.summary.files_changed', { count: summary.totalFiles })}
            {summary.additions > 0 && (
              <Text className="text-green-500"> +{summary.additions}</Text>
            )}
            {summary.deletions > 0 && (
              <Text className="text-red-500"> -{summary.deletions}</Text>
            )}
          </Text>
        </View>
      )}

      {/* Content */}
      <ScrollView className="flex-1" contentContainerStyle={{ paddingBottom: 24 }} nestedScrollEnabled={true}>
        {/* Loading state — skeleton */}
        {loading && (
          <View className="px-4 py-3">
            {[1, 2, 3].map((i) => (
              <View key={i} className="mb-3 flex-row items-center gap-2">
                <View className="h-4 w-4 animate-pulse rounded bg-muted" />
                <View className="h-4 flex-1 animate-pulse rounded bg-muted" />
                <View className="h-4 w-12 animate-pulse rounded bg-muted" />
              </View>
            ))}
          </View>
        )}

        {/* Error state */}
        {error && !loading && (
          <View className="items-center px-4 py-8">
            <Text className="mb-3 text-sm text-destructive">{error}</Text>
            <Pressable onPress={fetchStatus} className="rounded-lg bg-muted px-4 py-2">
              <Text className="text-sm font-medium">{t('common.retry')}</Text>
            </Pressable>
          </View>
        )}

        {/* Empty state */}
        {!loading && !error && files.length === 0 && summary && (
          <View className="items-center px-4 py-8">
            <Icon as={CheckCircleIcon} className="mb-2 text-green-500" size={32} />
            <Text className="text-sm text-muted-foreground">{t('diff.empty')}</Text>
          </View>
        )}

        {/* File list */}
        {!loading &&
          !error &&
          files.map((file) => {
            const statusInfo = STATUS_ICONS[file.status] || {
              label: '?',
              color: 'text-muted-foreground',
            };
            return (
              <Pressable
                key={file.path}
                onPress={() => onFileClick?.(file.path)}
                className="flex-row items-center gap-2 border-b border-border/50 px-4 py-2.5 active:bg-muted">
                <Text className={cn('w-5 text-center text-xs font-bold', statusInfo.color)}>
                  {statusInfo.label}
                </Text>
                <Text className="flex-1 text-sm" numberOfLines={1}>
                  {file.path}
                </Text>
                <View className="flex-row items-center gap-1">
                  {file.additions > 0 && (
                    <Text className="text-xs text-green-500">+{file.additions}</Text>
                  )}
                  {file.deletions > 0 && (
                    <Text className="text-xs text-red-500">-{file.deletions}</Text>
                  )}
                </View>
              </Pressable>
            );
          })}
      </ScrollView>
    </View>
  );
}
