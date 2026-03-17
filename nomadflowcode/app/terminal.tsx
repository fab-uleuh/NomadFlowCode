import { ArrowKeysBar } from '@/components/mobile/ArrowKeysBar';
import { DrawerPanel, WorktreeDrawerContent, DiffDrawerContent } from '@/components/mobile/DrawerPanel';
import { FileTreeDrawerContent } from '@/components/mobile/FileTreeDrawerContent';
import { MobileDiffView } from '@/components/mobile/MobileDiffView';
import { MiniBar } from '@/components/mobile/MiniBar';
import { Button } from '@/components/ui/button';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { ShortcutQuickBar, ShortcutFormModal, ShortcutsSection } from '@/components/terminal-shortcuts';
import { useStorage } from '@/lib/context/storage-context';
import { executeServerCommand } from '@/lib/server-commands';
import type { Feature } from '@shared';
import type { ConnectionState, TerminalShortcut } from '@/lib/types';
import type { NativeToWebMessage, WebToNativeMessage, Pane, CreatePaneRequest } from '@/lib/types/terminal-messages';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  KeyboardIcon,
  MenuIcon,
  LogOutIcon,
  MicIcon,
  PauseIcon,
  PlusIcon,
  RefreshCwIcon,
  SendIcon,
  WifiIcon,
  WifiOffIcon,
  XCircleIcon,
  XIcon,
} from 'lucide-react-native';
import { useColorScheme } from 'nativewind';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useTranslation } from 'react-i18next';
import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import {
  View,
  Pressable,
  Animated,
  AppState as RNAppState,
  BackHandler,
  KeyboardAvoidingView,
  PanResponder,
  Platform,
  Keyboard,
  TextInput,
  Modal,
} from 'react-native';
import { WebView, type WebViewMessageEvent } from 'react-native-webview';
import terminalHtml from '@/assets/terminal-html';

/** Parse a raw WebView message string into a typed WebToNativeMessage, with runtime validation.
 *  Returns null for unrecognized or malformed messages (e.g. scroll_state debug artifact). */
function parseWebViewMessage(data: string): WebToNativeMessage | null {
  let raw: Record<string, unknown>;
  try {
    raw = JSON.parse(data);
  } catch (e) {
    console.error('Failed to parse WebView message:', e);
    return null;
  }
  if (!raw || typeof raw.type !== 'string') return null;

  switch (raw.type) {
    case 'connected':
      return { type: 'connected' };
    case 'disconnected':
      return { type: 'disconnected' };
    case 'paneSwitched':
      return typeof raw.paneId === 'number'
        ? { type: 'paneSwitched', paneId: raw.paneId, label: String(raw.label ?? '') }
        : null;
    case 'paneList':
      return Array.isArray(raw.panes)
        ? { type: 'paneList', panes: raw.panes as Pane[] }
        : null;
    case 'paneDestroyed':
      return typeof raw.paneId === 'number'
        ? { type: 'paneDestroyed', paneId: raw.paneId }
        : null;
    case 'paneStateUpdated':
      return typeof raw.paneId === 'number' && typeof raw.agentState === 'string'
        ? { type: 'paneStateUpdated', paneId: raw.paneId, agentState: raw.agentState as any }
        : null;
    case 'resized':
      return typeof raw.cols === 'number' && typeof raw.rows === 'number'
        ? { type: 'resized', cols: raw.cols, rows: raw.rows }
        : null;
    case 'font_size':
      return typeof raw.fontSize === 'number'
        ? { type: 'font_size', fontSize: raw.fontSize }
        : null;
    case 'reconnecting':
      return typeof raw.attempt === 'number' && typeof raw.maxAttempts === 'number'
        ? { type: 'reconnecting', attempt: raw.attempt, maxAttempts: raw.maxAttempts }
        : null;
    case 'error':
      return typeof raw.message === 'string'
        ? { type: 'error', message: raw.message }
        : null;
    default:
      return null;
  }
}

const KEYBOARD_SHORTCUTS = [
  { label: 'Ctrl+C', char: '\x03', icon: XCircleIcon },
  { label: 'Ctrl+D', char: '\x04', icon: LogOutIcon },
  { label: 'Ctrl+Z', char: '\x1a', icon: PauseIcon },
  { label: 'Ctrl+L', char: '\x0c', icon: RefreshCwIcon },
  { label: 'Tab', char: '\t', icon: ArrowRightIcon },
  { label: 'Esc', char: '\x1b', icon: XIcon },
];

function EdgeSwipeZone({ side, onSwipe }: { side: 'left' | 'right'; onSwipe: () => void }) {
  const onSwipeRef = useRef(onSwipe);
  onSwipeRef.current = onSwipe;

  const panResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => false,
        onMoveShouldSetPanResponder: (_, gs) =>
          Math.abs(gs.dx) > 30 && Math.abs(gs.dx) > Math.abs(gs.dy),
        onPanResponderRelease: (_, gs) => {
          if (side === 'left' && gs.dx > 30) onSwipeRef.current();
          if (side === 'right' && gs.dx < -30) onSwipeRef.current();
        },
      }),
    [side]
  );

  return (
    <View
      style={{
        position: 'absolute',
        top: 0,
        bottom: 0,
        width: 20,
        ...(side === 'left' ? { left: 0 } : { right: 0 }),
        zIndex: 10,
      }}
      {...panResponder.panHandlers}
    />
  );
}

/** Derive the WS URL for the multiplexed pane endpoint from the API URL. */
function buildWsUrl(apiUrl: string): string {
  const base = apiUrl.replace(/\/+$/, '').replace(/\/api$/, '');
  const wsScheme = base.startsWith('https') ? 'wss' : 'ws';
  const host = base.replace(/^https?:\/\//, '');
  return `${wsScheme}://${host}/ws/panes`;
}

export default function TerminalScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const { colorScheme } = useColorScheme();
  const insets = useSafeAreaInsets();
  const params = useLocalSearchParams<{
    serverId: string;
    repoPath: string;
    featureName: string;
  }>();

  const { getServer, settings, updateSettings, updateServer, recentFeatures, terminalShortcuts, addTerminalShortcut, updateTerminalShortcut, deleteTerminalShortcut } = useStorage();

  const server = getServer(params.serverId);
  const featureName = params.featureName;
  const repoPath = params.repoPath;

  const feature = recentFeatures.find((f) => f.name === featureName) || {
    name: featureName,
    worktreePath: repoPath === '~' ? '~' : `${repoPath}/../worktrees/${featureName}`,
    branch: `feature/${featureName}`,
    isActive: true,
  };

  const webViewRef = useRef<WebView>(null);

  const shortcutsAnim = useRef(new Animated.Value(0)).current;
  const headerAnim = useRef(new Animated.Value(1)).current;

  const [connectionState, setConnectionState] = useState<ConnectionState>({
    status: 'connecting',
    reconnectAttempts: 0,
  });
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [showHeader, setShowHeader] = useState(true);
  const [shortcutModalVisible, setShortcutModalVisible] = useState(false);
  const [editingShortcut, setEditingShortcut] = useState<TerminalShortcut | null>(null);
  const [activePaneId, setActivePaneId] = useState<number | null>(null);
  const [panes, setPanes] = useState<Pane[]>([]);
  const [leftDrawerOpen, setLeftDrawerOpen] = useState(false);
  const [rightDrawerOpen, setRightDrawerOpen] = useState(false);
  const [selectedDiffFile, setSelectedDiffFile] = useState<string | null>(null);
  const [selectedDiffFileSource, setSelectedDiffFileSource] = useState<'diff' | 'tree'>('diff');
  const [rightDrawerTab, setRightDrawerTab] = useState<'changes' | 'files'>('changes');
  const [dictationVisible, setDictationVisible] = useState(false);
  const [dictationText, setDictationText] = useState('');
  const dictationInputRef = useRef<TextInput>(null);

  // Fetch all worktrees for the drawer
  const [allFeatures, setAllFeatures] = useState<Feature[]>([]);
  useEffect(() => {
    if (!server || !repoPath) return;
    executeServerCommand(server, {
      action: 'list-features',
      params: { repoPath },
    }).then((result) => {
      if (result.success && result.data?.features) {
        setAllFeatures(result.data.features);
      }
    }).catch(() => {});
  }, [server?.id, repoPath]);

  // Map all panes to SessionWithState format for MiniBar/drawer compatibility
  const allPaneSessions = useMemo(
    () => panes.map((p) => ({
      sessionId: String(p.id),
      windowName: p.label,
      repo: p.repo,
      worktree: p.worktree,
      agentType: p.agentType,
      agentNumber: p.agentNumber,
      agentState: p.agentState || 'unknown',
      stateTimestamp: null,
    })),
    [panes]
  );

  const paneSessions = useMemo(
    () => allPaneSessions.filter((s) => s.worktree === featureName),
    [allPaneSessions, featureName]
  );

  const activeSessionId = activePaneId != null ? String(activePaneId) : null;

  useEffect(() => {
    if (server) {
      updateServer(server.id, { lastConnected: Date.now() });
    }
  }, [server?.id, updateServer]);

  useEffect(() => {
    Animated.timing(shortcutsAnim, {
      toValue: showShortcuts ? 1 : 0,
      duration: 200,
      useNativeDriver: true,
    }).start();
  }, [showShortcuts]);

  useEffect(() => {
    Animated.timing(headerAnim, {
      toValue: showHeader ? 1 : 0,
      duration: 150,
      useNativeDriver: true,
    }).start();
  }, [showHeader]);

  /** Post a message to the bundled terminal HTML via WebView. */
  const postToWebView = useCallback((msg: NativeToWebMessage) => {
    webViewRef.current?.postMessage(JSON.stringify(msg));
  }, []);

  const sendToTerminal = useCallback((data: string) => {
    postToWebView({ type: 'sendInput', data });
  }, [postToWebView]);

  const openDictation = useCallback(() => {
    setDictationText('');
    setDictationVisible(true);
    // Blur WebView so native keyboard can take focus
    postToWebView({ type: 'blur' });
    setTimeout(() => dictationInputRef.current?.focus(), 100);
  }, [postToWebView]);

  const submitDictation = useCallback(() => {
    if (dictationText.trim()) {
      sendToTerminal(dictationText);
    }
    setDictationVisible(false);
    setDictationText('');
  }, [dictationText, sendToTerminal]);

  const cancelDictation = useCallback(() => {
    setDictationVisible(false);
    setDictationText('');
  }, []);

  const handleWebViewMessage = useCallback((event: WebViewMessageEvent) => {
    const message = parseWebViewMessage(event.nativeEvent.data);
    if (!message) return;

    switch (message.type) {
      case 'connected':
        setConnectionState({ status: 'connected', reconnectAttempts: 0 });
        break;
      case 'disconnected':
        setConnectionState((prev) => {
          // If currently reconnecting, don't overwrite with disconnected (transient during auto-reconnect)
          if (prev.status === 'reconnecting') return prev;
          return {
            status: 'disconnected',
            reconnectAttempts: prev.reconnectAttempts,
          };
        });
        break;
      case 'reconnecting':
        setConnectionState({
          status: 'reconnecting',
          reconnectAttempts: message.attempt,
        });
        break;
      case 'paneSwitched':
        setActivePaneId(message.paneId);
        break;
      case 'paneList':
        setPanes(message.panes);
        break;
      case 'paneDestroyed':
        setPanes((prev) => prev.filter((p) => p.id !== message.paneId));
        if (message.paneId === activePaneId) {
          setActivePaneId(null);
        }
        break;
      case 'paneStateUpdated':
        setPanes((prev) =>
          prev.map((p) =>
            p.id === message.paneId ? { ...p, agentState: message.agentState } : p
          )
        );
        break;
      case 'resized':
        break;
      case 'font_size':
        updateSettings({ fontSize: message.fontSize });
        break;
      case 'error':
        console.warn('[Terminal] WebView error:', message.message);
        setConnectionState({
          status: 'error',
          error: message.message,
          reconnectAttempts: 0,
        });
        break;
      default: {
        const _exhaustive: never = message;
        break;
      }
    }
  }, [activePaneId, updateSettings]);

  // Resize terminal when keyboard shows/hides
  useEffect(() => {
    const resizeTerminal = () => {
      setTimeout(() => postToWebView({ type: 'resize' }), 100);
    };
    const showSub = Keyboard.addListener('keyboardDidShow', resizeTerminal);
    const hideSub = Keyboard.addListener('keyboardDidHide', resizeTerminal);
    return () => { showSub.remove(); hideSub.remove(); };
  }, [postToWebView]);

  // iOS backgrounding: proactively close WS to prevent iOS from killing the app (AC #4)
  useEffect(() => {
    if (Platform.OS !== 'ios') return;
    const sub = RNAppState.addEventListener('change', (nextState) => {
      if (nextState === 'background') {
        postToWebView({ type: 'disconnect' });
      } else if (nextState === 'active') {
        postToWebView({ type: 'reconnect' });
      }
    });
    return () => sub.remove();
  }, [postToWebView]);

  const executeShortcut = useCallback((shortcut: TerminalShortcut) => {
    sendToTerminal(shortcut.command + (shortcut.autoExecute ? '\n' : ''));
  }, [sendToTerminal]);

  const handleAddShortcut = useCallback(() => {
    setEditingShortcut(null);
    setShortcutModalVisible(true);
  }, []);

  const handleEditShortcut = useCallback((shortcut: TerminalShortcut) => {
    setEditingShortcut(shortcut);
    setShortcutModalVisible(true);
  }, []);

  const handleSaveShortcut = useCallback(async (data: { label: string; command: string; autoExecute: boolean }) => {
    if (editingShortcut) {
      await updateTerminalShortcut(editingShortcut.id, data);
    } else {
      await addTerminalShortcut({ ...data, order: terminalShortcuts.length });
    }
    setShortcutModalVisible(false);
    setEditingShortcut(null);
  }, [editingShortcut, terminalShortcuts.length, addTerminalShortcut, updateTerminalShortcut]);

  const handleDeleteShortcut = useCallback(async () => {
    if (editingShortcut) {
      await deleteTerminalShortcut(editingShortcut.id);
      setShortcutModalVisible(false);
      setEditingShortcut(null);
    }
  }, [editingShortcut, deleteTerminalShortcut]);

  const handleSwitchSession = useCallback(
    (sessionId: string) => {
      const paneId = Number(sessionId);
      if (!isNaN(paneId)) {
        postToWebView({ type: 'switchPane', paneId });
      }
    },
    [postToWebView]
  );

  const handleSwitchWorktree = useCallback(
    (newWorktree: string) => {
      // Update URL params so the UI reflects the new worktree
      router.replace({
        pathname: '/terminal',
        params: { serverId: params.serverId, repoPath, featureName: newWorktree },
      } as any);

      // Find panes for the new worktree and switch to the first one
      const targetPanes = panes.filter((p) => p.worktree === newWorktree);
      if (targetPanes.length > 0) {
        postToWebView({ type: 'switchPane', paneId: targetPanes[0].id });
      }
    },
    [router, params.serverId, repoPath, panes, postToWebView]
  );

  const handleCreateSession = useCallback(
    (agentType: string) => {
      const worktreePath = feature.worktreePath;
      const repoBase = repoPath ? (repoPath.split('/').filter(Boolean).pop() || repoPath) : '';
      postToWebView({
        type: 'createPane',
        request: {
          repo: repoBase,
          worktree: featureName || '',
          agentType,
          cwd: worktreePath,
        },
      });
    },
    [feature.worktreePath, repoPath, featureName, postToWebView]
  );

  const handleDestroySession = useCallback(
    (sessionId: string) => {
      const paneId = Number(sessionId);
      if (!isNaN(paneId)) {
        postToWebView({ type: 'destroyPane', paneId });
      }
    },
    [postToWebView]
  );

  const openLeftDrawer = useCallback(() => {
    setRightDrawerOpen(false);
    setLeftDrawerOpen(true);
  }, []);

  const openRightDrawer = useCallback(() => {
    setLeftDrawerOpen(false);
    setRightDrawerOpen(true);
  }, []);

  const closeDrawers = useCallback(() => {
    setLeftDrawerOpen(false);
    setRightDrawerOpen(false);
  }, []);

  // Close diff view / drawers on Android back button
  useEffect(() => {
    if (Platform.OS !== 'android') return;
    const handler = BackHandler.addEventListener('hardwareBackPress', () => {
      if (selectedDiffFile) {
        setSelectedDiffFile(null);
        return true;
      }
      if (leftDrawerOpen || rightDrawerOpen) {
        closeDrawers();
        return true;
      }
      return false;
    });
    return () => handler.remove();
  }, [selectedDiffFile, leftDrawerOpen, rightDrawerOpen, closeDrawers]);

  const apiUrl = useMemo(
    () => (server?.apiUrl || 'http://localhost:8080').replace(/\/+$/, ''),
    [server?.apiUrl]
  );

  // Build the config to inject before content loads
  const injectedConfig = useMemo(() => {
    const wsUrl = buildWsUrl(apiUrl);
    const repoBasename = repoPath ? (repoPath.split('/').filter(Boolean).pop() || repoPath) : '';
    const paneLabel = repoBasename && featureName ? `${repoBasename}:${featureName}` : '';
    const agentType = settings.autoLaunchAgent ? (settings.defaultAiAgent || 'shell') : 'shell';
    return `
      window.__NOMADFLOW_CONFIG__ = ${JSON.stringify({
        wsUrl,
        token: server?.authToken || '',
        paneLabel,
        repo: repoBasename || '',
        worktree: featureName || '',
        agentType,
        cwd: feature.worktreePath || repoPath || '',
        fontSize: settings.fontSize,
        maxReconnectAttempts: settings.maxReconnectAttempts,
        reconnectDelay: settings.reconnectDelay,
        autoLaunchAgent: settings.autoLaunchAgent || false,
        defaultAiAgent: settings.defaultAiAgent || 'claude',
        customAgentCommand: settings.customAgentCommand || '',
      })};
      true;
    `;
  }, [apiUrl, server?.authToken, repoPath, featureName, feature.worktreePath, settings.fontSize, settings.maxReconnectAttempts, settings.reconnectDelay, settings.autoLaunchAgent, settings.defaultAiAgent, settings.customAgentCommand]);

  if (!server) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <Text className="text-destructive">{t('common.error.server_not_found')}</Text>
        <Button className="mt-4" onPress={() => router.back()}>
          <Text>{t('common.back')}</Text>
        </Button>
      </View>
    );
  }

  const isDark = colorScheme === 'dark';
  const bgColor = isDark ? 'hsl(240, 15%, 6%)' : 'hsl(240, 20%, 98%)';

  const statusColor = connectionState.status === 'connected' ? 'text-success' : connectionState.status === 'error' || connectionState.status === 'disconnected' ? 'text-destructive' : 'text-warning';

  return (
    <View className="flex-1 bg-black" style={{ paddingTop: insets.top }}>
      <Stack.Screen options={{ headerShown: false }} />
      <KeyboardAvoidingView
        className="flex-1"
        behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
      >

      {/* Header */}
      <Animated.View
        style={{
          opacity: headerAnim,
          transform: [{ translateY: headerAnim.interpolate({ inputRange: [0, 1], outputRange: [-60, 0] }) }],
        }}
        className="flex-row items-center border-b border-border bg-card px-2 py-2">
        <Button variant="ghost" size="sm" onPress={() => router.back()}>
          <Icon as={ArrowLeftIcon} size={20} />
          <Text className="ml-1">{t('common.back')}</Text>
        </Button>

        {Platform.OS !== 'web' && (
          <Pressable
            onPress={openLeftDrawer}
            style={{ minWidth: 44, minHeight: 44 }}
            className="items-center justify-center">
            <Icon as={MenuIcon} className="text-muted-foreground" size={20} />
          </Pressable>
        )}

        <View className="flex-1 items-center">
          <Text className="font-semibold" numberOfLines={1}>{featureName}</Text>
          <View className="flex-row items-center gap-1">
            <Icon as={connectionState.status === 'connected' ? WifiIcon : WifiOffIcon} className={statusColor} size={12} />
            <Text className={`text-xs ${statusColor}`}>
              {connectionState.status === 'reconnecting'
                ? t('terminal.status.reconnecting_count', { attempts: connectionState.reconnectAttempts, max: settings.maxReconnectAttempts })
                : t(`terminal.status.${connectionState.status}`)}
            </Text>
          </View>
        </View>

        {Platform.OS !== 'web' && (
          <Button variant="ghost" size="icon" onPress={openDictation}>
            <Icon as={MicIcon} size={20} />
          </Button>
        )}

        <Button variant="ghost" size="icon" onPress={() => {
          if (!showShortcuts) {
            postToWebView({ type: 'blur' });
            Keyboard.dismiss();
          }
          setShowShortcuts(!showShortcuts);
        }}>
          <Icon as={KeyboardIcon} size={22} />
        </Button>
      </Animated.View>

      {/* Terminal WebView — bundled HTML with multiplexed WS */}
      <View className="flex-1">
        <WebView
          ref={webViewRef}
          source={{ html: terminalHtml }}
          onMessage={handleWebViewMessage}
          onError={(syntheticEvent) => {
            const { nativeEvent } = syntheticEvent;
            console.warn('[Terminal] WebView error:', nativeEvent.description);
            setConnectionState({
              status: 'error',
              error: nativeEvent.description || t('terminal.error.load_failed'),
              reconnectAttempts: 0,
            });
          }}
          javaScriptEnabled
          domStorageEnabled
          originWhitelist={['*']}
          scrollEnabled={false}
          bounces={false}
          keyboardDisplayRequiresUserAction={false}
          injectedJavaScriptBeforeContentLoaded={injectedConfig}
          style={{ flex: 1, backgroundColor: bgColor }}
        />

        <Pressable onPress={() => setShowHeader(!showHeader)} className="absolute left-0 right-0 top-0 h-8" />

        {/* Edge swipe trigger zones for drawers */}
        {Platform.OS !== 'web' && (
          <>
            <EdgeSwipeZone side="left" onSwipe={openLeftDrawer} />
            <EdgeSwipeZone side="right" onSwipe={openRightDrawer} />
          </>
        )}

        {connectionState.status === 'error' && (
          <View className="absolute inset-0 items-center justify-center bg-black/90 p-8">
            <Icon as={WifiOffIcon} className="mb-4 text-muted-foreground" size={64} />
            <Text className="mb-2 text-xl font-semibold">
              {t('terminal.error.connection_title')}
            </Text>
            <Text className="mb-6 text-center text-muted-foreground">
              {connectionState.error || t('terminal.error.connection_lost')}
            </Text>
            <Button onPress={() => {
              setConnectionState({ status: 'connecting', reconnectAttempts: 0 });
              // Targeted reconnect: reset attempts and reopen WS (not full WebView reload)
              postToWebView({ type: 'reconnect' });
            }}>
              <Icon as={RefreshCwIcon} className="mr-2" size={18} />
              <Text>{t('terminal.reconnect')}</Text>
            </Button>
          </View>
        )}
      </View>

      {/* Quick Bar — below terminal, stays visible with and without keyboard */}
      <ShortcutQuickBar
        shortcuts={terminalShortcuts}
        onExecute={executeShortcut}
        onAdd={handleAddShortcut}
        onEdit={handleEditShortcut}
      />

      {/* Arrow Keys Bar — always visible on mobile */}
      {Platform.OS !== 'web' && (
        <ArrowKeysBar onSendKey={sendToTerminal} visible={true} />
      )}

      {/* MiniBar — inside KeyboardAvoidingView so Android keyboard doesn't overlap */}
      {Platform.OS !== 'web' && (
        <MiniBar
          sessions={paneSessions}
          allSessions={allPaneSessions}
          activeSessionId={activeSessionId}
          worktreeName={featureName}
          onSwitchSession={handleSwitchSession}
          onSwitchWorktree={handleSwitchWorktree}
          onCreateSession={handleCreateSession}
          onDestroySession={handleDestroySession}
        />
      )}

      </KeyboardAvoidingView>

      {/* Shortcuts Panel */}
      <Animated.View
        style={{
          opacity: shortcutsAnim,
          transform: [{ translateY: shortcutsAnim.interpolate({ inputRange: [0, 1], outputRange: [300, 0] }) }],
          position: 'absolute',
          bottom: 0,
          left: 0,
          right: 0,
          zIndex: 100,
        }}
        pointerEvents={showShortcuts ? 'auto' : 'none'}
        className="rounded-t-2xl bg-card p-4">
        <View className="mb-3 flex-row items-center justify-between">
          <Text className="font-semibold">{t('terminal.shortcuts.title')}</Text>
          <Pressable onPress={() => setShowShortcuts(false)} hitSlop={12}>
            <Icon as={XIcon} className="text-muted-foreground" size={20} />
          </Pressable>
        </View>

        <Text className="mb-2 text-xs text-muted-foreground">{t('terminal.shortcuts.special_keys')}</Text>
        <View className="mb-4 flex-row flex-wrap">
          {KEYBOARD_SHORTCUTS.map((s) => (
            <Pressable key={s.label} onPress={() => sendToTerminal(s.char)} className="w-1/4 items-center p-2">
              <View className="mb-1 h-10 w-10 items-center justify-center rounded-lg bg-background">
                <Icon as={s.icon} className="text-primary" size={18} />
              </View>
              <Text className="text-xs font-medium">{s.label}</Text>
            </Pressable>
          ))}
        </View>

        <ShortcutsSection
          shortcuts={terminalShortcuts}
          onExecute={executeShortcut}
          onAdd={handleAddShortcut}
          onEdit={handleEditShortcut}
        />
      </Animated.View>

      {/* Shortcut Form Modal */}
      <ShortcutFormModal
        visible={shortcutModalVisible}
        shortcut={editingShortcut}
        onSave={handleSaveShortcut}
        onDelete={editingShortcut ? handleDeleteShortcut : undefined}
        onClose={() => { setShortcutModalVisible(false); setEditingShortcut(null); }}
      />

      {/* Dictation Modal */}
      <Modal
        visible={dictationVisible}
        transparent
        animationType="slide"
        onRequestClose={cancelDictation}>
        <KeyboardAvoidingView
          style={{ flex: 1 }}
          behavior={Platform.OS === 'ios' ? 'padding' : 'height'}>
          <Pressable
            onPress={cancelDictation}
            style={{ flex: 1, justifyContent: 'flex-end', backgroundColor: 'rgba(0,0,0,0.5)' }}>
            <Pressable
              onPress={() => {}}
              style={{ backgroundColor: isDark ? '#1c1c24' : '#fff', borderTopLeftRadius: 16, borderTopRightRadius: 16, padding: 16 }}>
              <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
                <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                  <Icon as={MicIcon} className="text-primary" size={20} />
                  <Text className="font-semibold">{t('terminal.dictation.title', { defaultValue: 'Voice input' })}</Text>
                </View>
                <Pressable onPress={cancelDictation} hitSlop={12}>
                  <Icon as={XIcon} className="text-muted-foreground" size={20} />
                </Pressable>
              </View>
              <TextInput
                ref={dictationInputRef}
                value={dictationText}
                onChangeText={setDictationText}
                onSubmitEditing={submitDictation}
                placeholder={t('terminal.dictation.placeholder', { defaultValue: 'Tap mic on keyboard to dictate...' })}
                placeholderTextColor="#71717a"
                multiline
                autoFocus
                style={{
                  minHeight: 80,
                  borderWidth: 1,
                  borderColor: isDark ? '#333' : '#ddd',
                  borderRadius: 8,
                  padding: 12,
                  marginBottom: 12,
                  fontSize: 16,
                  color: isDark ? '#e4e4e7' : '#18181b',
                  backgroundColor: isDark ? '#0f0f17' : '#f4f4f5',
                  textAlignVertical: 'top',
                }}
              />
              <View style={{ flexDirection: 'row', gap: 8 }}>
                <Button variant="outline" className="flex-1" onPress={cancelDictation}>
                  <Text>{t('common.cancel', { defaultValue: 'Cancel' })}</Text>
                </Button>
                <Button className="flex-1" onPress={submitDictation} disabled={!dictationText.trim()}>
                  <Icon as={SendIcon} className="mr-2" size={16} />
                  <Text>{t('terminal.dictation.send', { defaultValue: 'Send' })}</Text>
                </Button>
              </View>
            </Pressable>
          </Pressable>
        </KeyboardAvoidingView>
      </Modal>

      {/* Swipe Drawers — mobile only */}
      {Platform.OS !== 'web' && (
        <>
          <DrawerPanel side="left" isOpen={leftDrawerOpen} onClose={closeDrawers}>
            <WorktreeDrawerContent
              allSessions={allPaneSessions}
              activeSessionId={activeSessionId}
              currentWorktree={featureName}
              features={allFeatures}
              onSwitchSession={handleSwitchSession}
              onSwitchWorktree={handleSwitchWorktree}
              onClose={closeDrawers}
            />
          </DrawerPanel>
          <DrawerPanel side="right" isOpen={rightDrawerOpen} onClose={closeDrawers}>
            <View className="flex-1">
              {/* Tab bar */}
              <View className="flex-row border-b border-border">
                <Pressable
                  onPress={() => setRightDrawerTab('changes')}
                  className={`flex-1 items-center py-2.5 ${
                    rightDrawerTab === 'changes' ? 'border-b-2 border-b-primary' : ''
                  }`}>
                  <Text className={`text-xs font-semibold uppercase ${
                    rightDrawerTab === 'changes' ? 'text-foreground' : 'text-muted-foreground'
                  }`}>
                    {t('file_tree.tab_changes')}
                  </Text>
                </Pressable>
                <Pressable
                  onPress={() => setRightDrawerTab('files')}
                  className={`flex-1 items-center py-2.5 ${
                    rightDrawerTab === 'files' ? 'border-b-2 border-b-primary' : ''
                  }`}>
                  <Text className={`text-xs font-semibold uppercase ${
                    rightDrawerTab === 'files' ? 'text-foreground' : 'text-muted-foreground'
                  }`}>
                    {t('file_tree.tab_files')}
                  </Text>
                </Pressable>
              </View>

              {/* Tab content */}
              {rightDrawerTab === 'changes' ? (
                <DiffDrawerContent
                  server={server}
                  worktreePath={feature.worktreePath}
                  isOpen={rightDrawerOpen}
                  onClose={closeDrawers}
                  onFileClick={(path) => {
                    setSelectedDiffFileSource('diff');
                    setSelectedDiffFile(path);
                  }}
                />
              ) : (
                <FileTreeDrawerContent
                  server={server}
                  worktreePath={feature.worktreePath}
                  isOpen={rightDrawerOpen}
                  onClose={closeDrawers}
                  onFileClick={(path) => {
                    setSelectedDiffFileSource('tree');
                    setSelectedDiffFile(path);
                  }}
                />
              )}
            </View>
          </DrawerPanel>
        </>
      )}

      {/* Diff file overlay — above drawers */}
      {Platform.OS !== 'web' && selectedDiffFile && (
        <MobileDiffView
          server={server}
          worktreePath={feature.worktreePath}
          filePath={selectedDiffFile}
          initialMode={selectedDiffFileSource === 'tree' ? 'file' : 'diff'}
          onClose={() => setSelectedDiffFile(null)}
        />
      )}
    </View>
  );
}
