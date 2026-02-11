import { Button } from '@/components/ui/button';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { ShortcutQuickBar, ShortcutFormModal, ShortcutsSection } from '@/components/terminal-shortcuts';
import { useStorage } from '@/lib/context/storage-context';
import { switchToFeature } from '@/lib/server-commands';
import type { SwitchFeatureResult } from '@shared';
import type { ConnectionState, TerminalShortcut } from '@/lib/types';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  ArrowUpIcon,
  ArrowDownIcon,
  ArrowLeft,
  KeyboardIcon,
  LayoutGridIcon,
  LogOutIcon,
  PauseIcon,
  PlusIcon,
  PowerIcon,
  RefreshCwIcon,
  ScrollIcon,
  SplitIcon,
  WifiIcon,
  WifiOffIcon,
  XCircleIcon,
  XIcon,
  ZoomInIcon,
  ZoomOutIcon,
} from 'lucide-react-native';
import { useColorScheme } from 'nativewind';
import { useState, useRef, useEffect, useCallback } from 'react';
import {
  View,
  Pressable,
  Animated,
  SafeAreaView,
  KeyboardAvoidingView,
  Platform,
  ActivityIndicator,
  Keyboard,
} from 'react-native';
import { WebView, type WebViewMessageEvent } from 'react-native-webview';

const ZOOM_MIN = 8;
const ZOOM_MAX = 56;
const ZOOM_STEP = 2;

/** Terminal page URL — derived from the API URL. */
const buildTerminalUrl = (server: { apiUrl?: string }): string => {
  const base = (server.apiUrl || 'http://localhost:8080').replace(/\/+$/, '');
  return `${base}/terminal`;
};

const KEYBOARD_SHORTCUTS = [
  { label: 'Ctrl+C', char: '\x03', icon: XCircleIcon },
  { label: 'Ctrl+D', char: '\x04', icon: LogOutIcon },
  { label: 'Ctrl+Z', char: '\x1a', icon: PauseIcon },
  { label: 'Ctrl+L', char: '\x0c', icon: RefreshCwIcon },
  { label: 'Tab', char: '\t', icon: ArrowRightIcon },
  { label: 'Esc', char: '\x1b', icon: XIcon },
  { label: '↑', char: '\x1b[A', icon: ArrowUpIcon },
  { label: '↓', char: '\x1b[B', icon: ArrowDownIcon },
];

const TMUX_SHORTCUTS = [
  { label: 'Windows', key: 'w', icon: LayoutGridIcon },
  { label: 'New', key: 'c', icon: PlusIcon },
  { label: 'Split H', key: '"', icon: SplitIcon },
  { label: 'Split V', key: '%', icon: SplitIcon },
  { label: 'Next', key: 'n', icon: ArrowRightIcon },
  { label: 'Prev', key: 'p', icon: ArrowLeft },
  { label: 'Detach', key: 'd', icon: PowerIcon },
  { label: 'Scroll', key: '[', icon: ScrollIcon },
];

/**
 * Build the JavaScript to inject before the ttyd page loads.
 * Intercepts WebSocket to:
 * 1. Rewrite URL to the API server's WS proxy (WKWebView can't send Basic Auth on WS upgrades)
 * 2. Track connection state via postMessage to React Native
 * 3. Expose sendInput() for keyboard shortcuts
 */
const buildInjectedJS = (apiUrl: string, authToken?: string): string => {
  const escapedToken = authToken ? authToken.replace(/\\/g, '\\\\').replace(/'/g, "\\'") : '';
  const escapedApiUrl = apiUrl.replace(/\\/g, '\\\\').replace(/'/g, "\\'");

  return `
(function() {
  var style = document.createElement('style');
  style.textContent = '.xterm-helper-textarea { caret-color: transparent !important; opacity: 0 !important; }';
  (document.head || document.documentElement).appendChild(style);

  var OriginalWebSocket = window.WebSocket;
  var _authToken = '${escapedToken}';
  var _apiUrl = '${escapedApiUrl}';

  window.WebSocket = function(url, protocols) {
    // Rewrite WebSocket URL to go through the API server's WS proxy
    // because WKWebView does not send Basic Auth on WebSocket upgrades
    var wsScheme = _apiUrl.indexOf('https') === 0 ? 'wss' : 'ws';
    var apiHost = _apiUrl.replace(/^https?:\\/\\//, '');
    url = wsScheme + '://' + apiHost + '/terminal/ws';
    if (_authToken) {
      url = url + '?token=' + encodeURIComponent(_authToken);
    }

    var ws = protocols ? new OriginalWebSocket(url, protocols) : new OriginalWebSocket(url);
    window._ttydSocket = ws;

    ws.addEventListener('open', function() {
      try { window.ReactNativeWebView.postMessage(JSON.stringify({ type: 'connected' })); } catch(e) {}
    });
    ws.addEventListener('close', function() {
      try { window.ReactNativeWebView.postMessage(JSON.stringify({ type: 'disconnected' })); } catch(e) {}
    });
    ws.addEventListener('error', function() {
      try { window.ReactNativeWebView.postMessage(JSON.stringify({ type: 'error', error: 'WebSocket connection failed' })); } catch(e) {}
    });

    return ws;
  };
  window.WebSocket.prototype = OriginalWebSocket.prototype;
  window.WebSocket.CONNECTING = OriginalWebSocket.CONNECTING;
  window.WebSocket.OPEN = OriginalWebSocket.OPEN;
  window.WebSocket.CLOSING = OriginalWebSocket.CLOSING;
  window.WebSocket.CLOSED = OriginalWebSocket.CLOSED;

  window.sendInput = function(data) {
    var ws = window._ttydSocket;
    if (ws && ws.readyState === 1) {
      var bytes = new Uint8Array(data.length + 1);
      bytes[0] = 48;
      for (var i = 0; i < data.length; i++) bytes[i + 1] = data.charCodeAt(i);
      ws.send(bytes.buffer);
      return true;
    }
    return false;
  };
})();
true;
`;
};

export default function TerminalScreen() {
  const router = useRouter();
  const { colorScheme } = useColorScheme();
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
  const hasRunningProcessRef = useRef(false);
  const shortcutsAnim = useRef(new Animated.Value(0)).current;
  const headerAnim = useRef(new Animated.Value(1)).current;

  const [connectionState, setConnectionState] = useState<ConnectionState>({
    status: 'connecting',
    reconnectAttempts: 0,
  });
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [currentFontSize, setCurrentFontSize] = useState(settings.fontSize);
  const [showHeader, setShowHeader] = useState(true);
  const [isPreparingTerminal, setIsPreparingTerminal] = useState(true);
  const [actualWorktreePath, setActualWorktreePath] = useState<string | null>(null);
  const [hasRunningProcess, setHasRunningProcess] = useState(false);
  const [shortcutModalVisible, setShortcutModalVisible] = useState(false);
  const [editingShortcut, setEditingShortcut] = useState<TerminalShortcut | null>(null);

  // Prépare le terminal (switch feature via API)
  useEffect(() => {
    if (!server || !repoPath || !featureName) {
      setIsPreparingTerminal(false);
      return;
    }

    (async () => {
      try {
        const result = await switchToFeature(server, { repoPath, featureName });
        if (result.success && result.data) {
          setActualWorktreePath(result.data.worktreePath ?? null);
          const running = !!result.data.hasRunningProcess;
          setHasRunningProcess(running);
          hasRunningProcessRef.current = running;
        }
      } catch (error) {
        console.warn('[Terminal] Error switching feature:', error);
      } finally {
        setIsPreparingTerminal(false);
      }
    })();
  }, [server?.id, repoPath, featureName]);

  useEffect(() => {
    if (server) {
      updateServer(server.id, { lastConnected: Date.now() });
    }
  }, [server?.id]);

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

  const sendToTerminal = useCallback((data: string) => {
    webViewRef.current?.injectJavaScript(`window.sendInput(${JSON.stringify(data)});true;`);
  }, []);

  const sendTmuxKey = useCallback((key: string) => {
    sendToTerminal('\x02' + key); // Ctrl-b + key
  }, [sendToTerminal]);

  const changeFontSize = useCallback(async (delta: number) => {
    const newSize = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, currentFontSize + delta));
    if (newSize !== currentFontSize) {
      setCurrentFontSize(newSize);
      webViewRef.current?.injectJavaScript(`
        if (window.term) {
          window.term.options.fontSize = ${newSize};
          window.dispatchEvent(new Event('resize'));
        }
        true;
      `);
      await updateSettings({ fontSize: newSize });
    }
  }, [currentFontSize, updateSettings]);

  const handleWebViewMessage = useCallback((event: WebViewMessageEvent) => {
    try {
      const message = JSON.parse(event.nativeEvent.data);
      switch (message.type) {
        case 'connected':
          setConnectionState({ status: 'connected', reconnectAttempts: 0 });
          sendInitCommands();
          break;
        case 'disconnected':
          setConnectionState((prev) => ({ status: 'disconnected', reconnectAttempts: prev.reconnectAttempts }));
          if (settings.autoReconnect) attemptReconnect();
          break;
        case 'error':
          setConnectionState({ status: 'error', error: message.error, reconnectAttempts: 0 });
          break;
      }
    } catch (error) {
      console.error('Failed to parse WebView message:', error);
    }
  }, [settings.autoReconnect]);

  const sendInitCommands = useCallback(() => {
    // Don't send any commands if a process (like claude) is already running
    // The server already detected this and skipped cd/clear
    // Use ref to always read the latest value (avoids stale closure in handleWebViewMessage)
    if (hasRunningProcessRef.current) {
      console.log('[Terminal] Process already running, skipping init commands');
      return;
    }

    setTimeout(() => {
      const worktreePath = actualWorktreePath || feature.worktreePath;
      if (settings.autoLaunchAgent) {
        const agentCommand = settings.defaultAiAgent === 'claude' ? 'claude'
          : settings.defaultAiAgent === 'ollama' ? 'ollama run deepseek-coder'
          : settings.customAgentCommand || 'echo "No agent configured"';
        sendToTerminal(agentCommand + '\n');
      } else {
        sendToTerminal(`echo "🚀 NomadFlow - ${featureName}"\n`);
        setTimeout(() => sendToTerminal(`echo "📂 ${worktreePath}"\n`), 300);
      }
    }, 500);
  }, [actualWorktreePath, featureName, feature, settings, sendToTerminal, hasRunningProcess]);

  const attemptReconnect = useCallback(() => {
    if (connectionState.reconnectAttempts >= settings.maxReconnectAttempts) {
      setConnectionState({ status: 'error', error: 'Maximum reconnection attempts reached', reconnectAttempts: connectionState.reconnectAttempts });
      return;
    }
    setTimeout(() => webViewRef.current?.reload(), settings.reconnectDelay);
  }, [connectionState.reconnectAttempts, settings]);

  // Resize xterm when keyboard shows/hides so it reflows to the visible area
  useEffect(() => {
    const resizeTerminal = () => {
      // Small delay to let KeyboardAvoidingView finish its layout adjustment
      setTimeout(() => {
        webViewRef.current?.injectJavaScript(`
          window.dispatchEvent(new Event('resize'));
          true;
        `);
      }, 100);
    };
    const showSub = Keyboard.addListener('keyboardDidShow', resizeTerminal);
    const hideSub = Keyboard.addListener('keyboardDidHide', resizeTerminal);
    return () => { showSub.remove(); hideSub.remove(); };
  }, []);

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

  if (!server) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <Text className="text-destructive">Serveur non trouvé</Text>
        <Button className="mt-4" onPress={() => router.back()}>
          <Text>Retour</Text>
        </Button>
      </View>
    );
  }

  const isDark = colorScheme === 'dark';
  const bgColor = isDark ? 'hsl(240, 15%, 6%)' : 'hsl(240, 20%, 98%)';
  const terminalUrl = buildTerminalUrl(server);
  const apiUrl = (server.apiUrl || 'http://localhost:8080').replace(/\/+$/, '');
  const injectedJS = buildInjectedJS(apiUrl, server.authToken);
  const basicAuthCredential = server.authToken
    ? { username: 'nomadflow', password: server.authToken }
    : undefined;

  console.log('[Terminal] terminal URL:', terminalUrl, 'API WS proxy:', apiUrl);
  const statusColor = connectionState.status === 'connected' ? 'text-success' : connectionState.status === 'error' || connectionState.status === 'disconnected' ? 'text-destructive' : 'text-warning';

  if (isPreparingTerminal) {
    return (
      <SafeAreaView className="flex-1 bg-black">
        <Stack.Screen options={{ headerShown: false }} />
        <View className="flex-1 items-center justify-center">
          <ActivityIndicator size="large" color="hsl(250, 85%, 65%)" />
          <Text className="mt-4 text-muted-foreground">Préparation du terminal...</Text>
          <Text className="mt-2 text-sm text-muted-foreground">{featureName}</Text>
        </View>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView className="flex-1 bg-black">
      <Stack.Screen options={{ headerShown: false }} />
      <KeyboardAvoidingView
        className="flex-1"
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
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
          <Text className="ml-1">Retour</Text>
        </Button>

        <View className="flex-1 items-center">
          <Text className="font-semibold" numberOfLines={1}>{featureName}</Text>
          <View className="flex-row items-center gap-1">
            <Icon as={connectionState.status === 'connected' ? WifiIcon : WifiOffIcon} className={statusColor} size={12} />
            <Text className={`text-xs ${statusColor}`}>{connectionState.status}</Text>
          </View>
        </View>

        <View className="flex-row items-center">
          <Button variant="ghost" size="icon" onPress={() => changeFontSize(-ZOOM_STEP)} disabled={currentFontSize <= ZOOM_MIN}>
            <Icon as={ZoomOutIcon} size={20} />
          </Button>
          <Text className="w-8 text-center text-xs text-muted-foreground">{currentFontSize}</Text>
          <Button variant="ghost" size="icon" onPress={() => changeFontSize(ZOOM_STEP)} disabled={currentFontSize >= ZOOM_MAX}>
            <Icon as={ZoomInIcon} size={20} />
          </Button>
        </View>

        <Button variant="ghost" size="icon" onPress={() => {
          if (!showShortcuts) {
            // Fermer le clavier du WebView en retirant le focus
            webViewRef.current?.injectJavaScript(`
              if (document.activeElement) {
                document.activeElement.blur();
              }
              true;
            `);
            Keyboard.dismiss();
          }
          setShowShortcuts(!showShortcuts);
        }}>
          <Icon as={KeyboardIcon} size={22} />
        </Button>
      </Animated.View>

      {/* Quick Bar */}
      <ShortcutQuickBar
        shortcuts={terminalShortcuts}
        onExecute={executeShortcut}
        onAdd={handleAddShortcut}
        onEdit={handleEditShortcut}
      />

      {/* Terminal WebView */}
      <View className="flex-1">
        <WebView
          ref={webViewRef}
          source={{ uri: terminalUrl }}
          basicAuthCredential={basicAuthCredential}
          onMessage={handleWebViewMessage}
          onLoadEnd={() => {
            // Page loaded directly from ttyd (auth via basicAuthCredential).
            // WebSocket goes through API server proxy (injected JS rewrites URL).
            // The actual 'connected' state will be set by the WebSocket 'open'
            // event from the injected JS. Fallback timeout clears 'connecting'.
            if (connectionState.status === 'connecting') {
              setTimeout(() => {
                setConnectionState((prev) =>
                  prev.status === 'connecting'
                    ? { status: 'connected', reconnectAttempts: 0 }
                    : prev
                );
              }, 3000);
            }
          }}
          onHttpError={(syntheticEvent) => {
            const { statusCode } = syntheticEvent.nativeEvent;
            console.log('[Terminal] HTTP error:', statusCode);
            if (statusCode === 401) {
              setConnectionState({
                status: 'error',
                error: 'Authentification échouée — vérifiez le token',
                reconnectAttempts: 0,
              });
            }
          }}
          onError={(syntheticEvent) => {
            const { nativeEvent } = syntheticEvent;
            console.log('[Terminal] WebView error:', nativeEvent.description);
            setConnectionState({
              status: 'error',
              error: nativeEvent.description || 'Failed to load terminal',
              reconnectAttempts: 0,
            });
          }}
          javaScriptEnabled
          domStorageEnabled
          originWhitelist={['*']}
          scrollEnabled={false}
          bounces={false}
          keyboardDisplayRequiresUserAction={false}
          injectedJavaScriptBeforeContentLoaded={injectedJS}
          style={{ flex: 1, backgroundColor: bgColor }}
        />

        <Pressable onPress={() => setShowHeader(!showHeader)} className="absolute left-0 right-0 top-0 h-8" />

        {(connectionState.status === 'error' || connectionState.status === 'disconnected') && (
          <View className="absolute inset-0 items-center justify-center bg-black/90 p-8">
            <Icon as={WifiOffIcon} className="mb-4 text-muted-foreground" size={64} />
            <Text className="mb-2 text-xl font-semibold">
              {connectionState.status === 'error' ? 'Erreur de connexion' : 'Déconnecté'}
            </Text>
            <Text className="mb-6 text-center text-muted-foreground">
              {connectionState.error || 'La connexion au serveur a été perdue'}
            </Text>
            <Button onPress={() => { setConnectionState({ status: 'connecting', reconnectAttempts: 0 }); webViewRef.current?.reload(); }}>
              <Icon as={RefreshCwIcon} className="mr-2" size={18} />
              <Text>Reconnecter</Text>
            </Button>
          </View>
        )}
      </View>

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
          <Text className="font-semibold">Raccourcis clavier</Text>
          <Pressable onPress={() => setShowShortcuts(false)}>
            <Icon as={XIcon} className="text-muted-foreground" size={20} />
          </Pressable>
        </View>

        <Text className="mb-2 text-xs text-muted-foreground">Touches spéciales</Text>
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

        <Text className="mb-2 text-xs text-muted-foreground">tmux (Ctrl-b + ...)</Text>
        <View className="mb-4 flex-row flex-wrap">
          {TMUX_SHORTCUTS.map((s) => (
            <Pressable key={s.key} onPress={() => sendTmuxKey(s.key)} className="w-1/4 items-center p-2">
              <View className="mb-1 h-10 w-10 items-center justify-center rounded-lg bg-background">
                <Icon as={s.icon} className="text-primary" size={18} />
              </View>
              <Text className="text-xs font-medium">{s.label}</Text>
              <Text className="text-[10px] text-muted-foreground">^b {s.key}</Text>
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
    </SafeAreaView>
  );
}
