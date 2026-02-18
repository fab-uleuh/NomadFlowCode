import { ArrowKeysBar } from '@/components/mobile/ArrowKeysBar';
import { DrawerPanel, WorktreeDrawerContent, DiffDrawerContent } from '@/components/mobile/DrawerPanel';
import { MobileDiffView } from '@/components/mobile/MobileDiffView';
import { MiniBar } from '@/components/mobile/MiniBar';
import { Button } from '@/components/ui/button';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { ShortcutQuickBar, ShortcutFormModal, ShortcutsSection } from '@/components/terminal-shortcuts';
import { useStorage } from '@/lib/context/storage-context';
import { useAgentStatePolling } from '@/lib/hooks/useAgentStatePolling';
import { executeServerCommand, switchToFeature } from '@/lib/server-commands';
import type { Feature } from '@shared';
import type { ConnectionState, TerminalShortcut } from '@/lib/types';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  ArrowLeft,
  KeyboardIcon,
  LayoutGridIcon,
  MenuIcon,
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
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useTranslation } from 'react-i18next';
import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import {
  View,
  Pressable,
  Animated,
  BackHandler,
  KeyboardAvoidingView,
  PanResponder,
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
];

const TMUX_SHORTCUTS = [
  { labelKey: 'terminal.shortcuts.tmux.windows', key: 'w', icon: LayoutGridIcon },
  { labelKey: 'terminal.shortcuts.tmux.new', key: 'c', icon: PlusIcon },
  { labelKey: 'terminal.shortcuts.tmux.split_h', key: '"', icon: SplitIcon },
  { labelKey: 'terminal.shortcuts.tmux.split_v', key: '%', icon: SplitIcon },
  { labelKey: 'terminal.shortcuts.tmux.next', key: 'n', icon: ArrowRightIcon },
  { labelKey: 'terminal.shortcuts.tmux.prev', key: 'p', icon: ArrowLeft },
  { labelKey: 'terminal.shortcuts.tmux.detach', key: 'd', icon: PowerIcon },
  { labelKey: 'terminal.shortcuts.tmux.scroll', key: '[', icon: ScrollIcon },
];

/**
 * Build the JavaScript to inject before the ttyd page loads.
 * Intercepts WebSocket to:
 * 1. Rewrite URL to the API server's WS proxy (WKWebView can't send Basic Auth on WS upgrades)
 * 2. Track connection state via postMessage to React Native
 * 3. Expose sendInput() for keyboard shortcuts
 */
const buildInjectedJS = (apiUrl: string, authToken?: string, windowName?: string): string => {
  const escapedToken = authToken ? authToken.replace(/\\/g, '\\\\').replace(/'/g, "\\'") : '';
  const escapedApiUrl = apiUrl.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
  const escapedWindow = windowName ? windowName.replace(/\\/g, '\\\\').replace(/'/g, "\\'") : '';

  return `
(function() {
  var style = document.createElement('style');
  style.textContent = '.xterm-helper-textarea { caret-color: transparent !important; opacity: 0 !important; }';
  (document.head || document.documentElement).appendChild(style);

  // Prevent native WebView pinch-to-zoom (we handle font size ourselves)
  var meta = document.createElement('meta');
  meta.name = 'viewport';
  meta.content = 'width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no';
  (document.head || document.documentElement).appendChild(meta);

  var OriginalWebSocket = window.WebSocket;
  var _authToken = '${escapedToken}';
  var _apiUrl = '${escapedApiUrl}';
  var _window = '${escapedWindow}';

  window.WebSocket = function(url, protocols) {
    // Rewrite WebSocket URL to go through the API server's WS proxy
    // because WKWebView does not send Basic Auth on WebSocket upgrades
    var wsScheme = _apiUrl.indexOf('https') === 0 ? 'wss' : 'ws';
    var apiHost = _apiUrl.replace(/^https?:\\/\\//, '');
    url = wsScheme + '://' + apiHost + '/terminal/ws';
    var params = [];
    if (_authToken) params.push('token=' + encodeURIComponent(_authToken));
    if (_window) params.push('window=' + encodeURIComponent(_window));
    if (params.length > 0) url = url + '?' + params.join('&');

    var ws = protocols ? new OriginalWebSocket(url, protocols) : new OriginalWebSocket(url);
    window._ttydSocket = ws;

    // Intercept text messages from server (linked session metadata).
    // Added BEFORE ttyd's own handler. Text messages are ignored by ttyd (it expects binary).
    ws.addEventListener('message', function(event) {
      if (typeof event.data === 'string') {
        try {
          var meta = JSON.parse(event.data);
          if (meta.linkedSession) {
            window._linkedSession = meta.linkedSession;
            window.ReactNativeWebView.postMessage(JSON.stringify({
              type: 'linked_session',
              name: meta.linkedSession
            }));
          }
        } catch(e) {}
      }
    });

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

  // === TouchScrollHandler ===
  // Custom touch scroll for xterm.js (no native mobile touch support until v7)
  // State machine: IDLE → TRACKING → SCROLLING → MOMENTUM → IDLE
  // In alternate buffer (tmux): enters tmux copy mode (Ctrl+b [) then sends arrow keys.
  // Tap exits copy mode. Keyboard show exits copy mode.
  (function() {
    var DEFAULT_LINE_HEIGHT = 20;
    var MOVE_THRESHOLD = 10;
    var DECAY_FACTOR = 0.95;
    var MIN_VELOCITY = 0.5;
    var VELOCITY_SAMPLES = 5;
    var VELOCITY_THRESHOLD = 0.05;

    function getLineHeight() {
      try {
        if (window.term && window.term._core && window.term._core._renderService) {
          return window.term._core._renderService.dimensions.css.cell.height || DEFAULT_LINE_HEIGHT;
        }
      } catch(e) {}
      return DEFAULT_LINE_HEIGHT;
    }

    var state = 'IDLE';
    var startX = 0;
    var startY = 0;
    var lastY = 0;
    var touchId = null;
    var directionLocked = false;
    var isVertical = false;
    var momentumId = null;
    var userScrolledUp = false;

    // Tmux copy mode tracking
    var inTmuxCopyMode = false;

    // NOTE: assumes default tmux prefix Ctrl+b (\x02). Custom prefixes are not supported.
    function enterTmuxCopyMode() {
      if (!inTmuxCopyMode) {
        window.sendInput('\\x02['); // Ctrl+b [ enters tmux copy mode
        inTmuxCopyMode = true;
      }
    }

    function exitTmuxCopyMode() {
      if (inTmuxCopyMode) {
        window.sendInput('\\x1b'); // Escape exits tmux copy mode (safer than 'q' if state desyncs)
        inTmuxCopyMode = false;
      }
    }

    // Velocity tracking
    var velocitySamples = [];

    function addVelocitySample(y, time) {
      velocitySamples.push({ y: y, time: time });
      if (velocitySamples.length > VELOCITY_SAMPLES) velocitySamples.shift();
    }

    function calculateVelocity() {
      if (velocitySamples.length < 2) return 0;
      var first = velocitySamples[0];
      var last = velocitySamples[velocitySamples.length - 1];
      var dt = last.time - first.time;
      if (dt === 0) return 0;
      return (last.y - first.y) / dt; // px/ms
    }

    function cancelMomentum() {
      if (momentumId) {
        cancelAnimationFrame(momentumId);
        momentumId = null;
      }
    }

    function startMomentum(velocityPxMs) {
      if (!window.term) return;
      var isAlternate = window.term.buffer.active.type !== 'normal';
      if (isAlternate && !inTmuxCopyMode) return;
      var frameTime = 16; // ~16ms per frame at 60fps
      var velocity = velocityPxMs * frameTime;
      var lineHeight = getLineHeight();
      function tick() {
        velocity *= DECAY_FACTOR;
        var lines = velocity / lineHeight;
        if (Math.abs(lines) < MIN_VELOCITY) {
          momentumId = null;
          return;
        }
        if (inTmuxCopyMode) {
          // Send arrow keys in tmux copy mode, capped to avoid flooding
          var count = Math.min(10, Math.max(1, Math.round(Math.abs(lines))));
          var seq = lines < 0 ? '\\x1b[A' : '\\x1b[B';
          for (var j = 0; j < count; j++) window.sendInput(seq);
        } else {
          window.term.scrollLines(Math.round(lines));
          updateScrollState();
        }
        momentumId = requestAnimationFrame(tick);
      }
      momentumId = requestAnimationFrame(tick);
    }

    function updateScrollState() {
      if (!window.term) return;
      var buf = window.term.buffer.active;
      var atBottom = buf.viewportY >= buf.baseY;
      if (atBottom && userScrolledUp) {
        userScrolledUp = false;
        try { window.ReactNativeWebView.postMessage(JSON.stringify({ type: 'scroll_state', atBottom: true })); } catch(e) {}
      } else if (!atBottom && !userScrolledUp) {
        userScrolledUp = true;
        try { window.ReactNativeWebView.postMessage(JSON.stringify({ type: 'scroll_state', atBottom: false })); } catch(e) {}
      }
    }

    function resetState() {
      state = 'IDLE';
      touchId = null;
      directionLocked = false;
      isVertical = false;
      velocitySamples = [];
    }

    document.addEventListener('touchstart', function(e) {
      // Ignore multi-touch (pinch zoom)
      if (e.touches.length > 1) {
        cancelMomentum();
        exitTmuxCopyMode();
        resetState();
        return;
      }

      cancelMomentum();
      var touch = e.touches[0];
      touchId = touch.identifier;
      startX = touch.clientX;
      startY = touch.clientY;
      lastY = touch.clientY;
      directionLocked = false;
      isVertical = false;
      velocitySamples = [];
      state = 'TRACKING';
    }, { passive: true });

    document.addEventListener('touchmove', function(e) {
      if (state !== 'TRACKING' && state !== 'SCROLLING') return;
      if (e.touches.length > 1) {
        resetState();
        return;
      }

      var touch = null;
      for (var i = 0; i < e.touches.length; i++) {
        if (e.touches[i].identifier === touchId) { touch = e.touches[i]; break; }
      }
      if (!touch) return;

      var deltaY = touch.clientY - startY;

      if (state === 'TRACKING') {
        var absDx = Math.abs(touch.clientX - startX);
        var absDy = Math.abs(deltaY);

        // Direction lock: decide once
        if (!directionLocked && (absDy > MOVE_THRESHOLD || absDx > MOVE_THRESHOLD)) {
          directionLocked = true;
          isVertical = absDy >= absDx;
        }

        if (!directionLocked) return;
        if (!isVertical) return; // Horizontal → ignore

        state = 'SCROLLING';
        lastY = touch.clientY; // Reset to avoid initial jump from threshold accumulation
      }

      // SCROLLING state
      e.preventDefault();
      var moveDelta = touch.clientY - lastY;
      lastY = touch.clientY;
      addVelocitySample(touch.clientY, e.timeStamp);

      if (!window.term) return;

      var currentLineHeight = getLineHeight();
      if (window.term.buffer.active.type === 'alternate') {
        // Alternate buffer (tmux uses alternate mode).
        // Enter tmux copy mode, then send arrow keys to scroll.
        enterTmuxCopyMode();
        var lines = Math.round(Math.abs(moveDelta) / currentLineHeight);
        if (lines < 1) return;
        var seq = moveDelta < 0 ? '\\x1b[A' : '\\x1b[B';
        for (var j = 0; j < lines; j++) window.sendInput(seq);
      } else {
        // Normal buffer: scroll directly
        var scrollLines = Math.round(moveDelta / currentLineHeight);
        if (scrollLines !== 0) {
          window.term.scrollLines(scrollLines);
          updateScrollState();
        }
      }
    }, { passive: false });

    document.addEventListener('touchend', function(e) {
      if (state === 'SCROLLING') {
        var velocity = calculateVelocity();
        var allowMomentum = window.term && (window.term.buffer.active.type === 'normal' || inTmuxCopyMode);
        if (Math.abs(velocity) > VELOCITY_THRESHOLD && allowMomentum) {
          state = 'MOMENTUM';
          startMomentum(velocity);
        }
      } else if (state === 'TRACKING' && inTmuxCopyMode) {
        // Tap (no scroll movement) while in copy mode → exit copy mode
        exitTmuxCopyMode();
      }
      resetState();
    }, { passive: true });

    document.addEventListener('touchcancel', function() {
      cancelMomentum();
      resetState();
    }, { passive: true });

    // Auto-scroll: track scroll position to know if user is at bottom
    if (window.term) {
      window.term.onScroll(function() { updateScrollState(); });
    } else {
      // term may not exist yet, wait for it
      var termCheckInterval = setInterval(function() {
        if (window.term) {
          clearInterval(termCheckInterval);
          window.term.onScroll(function() { updateScrollState(); });
        }
      }, 200);
    }

    // Cancel momentum + exit copy mode on keyboard show or font size change (resize event).
    // Note: pinch-to-zoom also triggers resize — copy mode exits during pinch (acceptable UX).
    window.addEventListener('resize', function() {
      cancelMomentum();
      exitTmuxCopyMode();
    });
  })();

  // === PinchZoomHandler ===
  // Custom pinch-to-zoom: changes terminal font size instead of native WebView zoom.
  // Dispatches resize event so tmux reflows to the new cell dimensions.
  (function() {
    var PINCH_MIN_FONT = ${ZOOM_MIN};
    var PINCH_MAX_FONT = ${ZOOM_MAX};
    var pinchInitialDistance = 0;
    var pinchInitialFontSize = 0;
    var pinchActive = false;

    function getPinchDistance(touches) {
      var dx = touches[0].clientX - touches[1].clientX;
      var dy = touches[0].clientY - touches[1].clientY;
      return Math.sqrt(dx * dx + dy * dy);
    }

    document.addEventListener('touchstart', function(e) {
      if (e.touches.length === 2 && window.term) {
        pinchActive = true;
        pinchInitialDistance = getPinchDistance(e.touches);
        pinchInitialFontSize = window.term.options.fontSize || PINCH_MIN_FONT;
      }
    }, { passive: true });

    document.addEventListener('touchmove', function(e) {
      if (!pinchActive || e.touches.length !== 2 || !window.term || pinchInitialDistance === 0) return;
      e.preventDefault();
      var currentDistance = getPinchDistance(e.touches);
      var scale = currentDistance / pinchInitialDistance;
      var newSize = Math.round(pinchInitialFontSize * scale);
      newSize = Math.max(PINCH_MIN_FONT, Math.min(PINCH_MAX_FONT, newSize));
      if (newSize !== window.term.options.fontSize) {
        window.term.options.fontSize = newSize;
        window.dispatchEvent(new Event('resize'));
      }
    }, { passive: false });

    document.addEventListener('touchend', function(e) {
      if (e.touches.length < 2 && pinchActive) {
        // Save font size only once at end of pinch gesture
        pinchActive = false;
        pinchInitialDistance = 0;
        if (window.term) {
          try {
            window.ReactNativeWebView.postMessage(JSON.stringify({ type: 'font_size', size: window.term.options.fontSize }));
          } catch(err) {}
        }
      }
    }, { passive: true });
  })();
})();
true;
`;
};

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
  const hasRunningProcessRef = useRef(false);

  const triggerTerminalRedraw = useCallback(() => {
    webViewRef.current?.injectJavaScript(`
      if (window.term) {
        var cols = window.term.cols;
        var rows = window.term.rows;
        window.term.resize(cols, rows - 1);
        setTimeout(function() { window.term.resize(cols, rows); }, 50);
      }
      true;
    `);
  }, []);

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
  const [shortcutModalVisible, setShortcutModalVisible] = useState(false);
  const [editingShortcut, setEditingShortcut] = useState<TerminalShortcut | null>(null);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const linkedSessionRef = useRef<string | null>(null);
  const [linkedSessionName, setLinkedSessionName] = useState<string | null>(null);
  const [keyboardVisible, setKeyboardVisible] = useState(false);
  const [leftDrawerOpen, setLeftDrawerOpen] = useState(false);
  const [rightDrawerOpen, setRightDrawerOpen] = useState(false);
  const [selectedDiffFile, setSelectedDiffFile] = useState<string | null>(null);

  // Poll sessions for MiniBar
  const { sessions: allSessions } = useAgentStatePolling(
    server?.apiUrl || '',
    server?.authToken || '',
    { interval: 3000 }
  );

  // Fetch all worktrees for the drawer (so it shows worktrees even without agents)
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

  // Filter sessions to current worktree
  const worktreeSessions = useMemo(
    () => allSessions.filter((s) => s.worktree === featureName),
    [allSessions, featureName]
  );

  // Initialize/update active session when sessions change
  useEffect(() => {
    if (worktreeSessions.length === 0) return;
    if (!activeSessionId || !worktreeSessions.find((s) => s.sessionId === activeSessionId)) {
      setActiveSessionId(worktreeSessions[0].sessionId);
    }
  }, [worktreeSessions, activeSessionId]);

  // Prépare le terminal (switch feature via API)
  const isFirstMount = useRef(true);
  useEffect(() => {
    if (!server || !repoPath || !featureName) {
      setIsPreparingTerminal(false);
      return;
    }

    const isSubsequentSwitch = !isFirstMount.current;
    isFirstMount.current = false;

    (async () => {
      try {
        const result = await switchToFeature(server, { repoPath, featureName, linkedSession: linkedSessionRef.current || undefined });
        if (result.success && result.data) {
          setActualWorktreePath(result.data.worktreePath ?? null);
          hasRunningProcessRef.current = !!result.data.hasRunningProcess;
        }
        // On subsequent switches (not initial mount), force a tmux redraw via resize cycle.
        // The WebView stays mounted so the ttyd client may not repaint after select-window.
        if (isSubsequentSwitch) {
          triggerTerminalRedraw();
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

  const sendInitCommands = useCallback(() => {
    // Don't send any commands if a process (like claude) is already running
    // The server already detected this and skipped cd/clear
    // Use ref to always read the latest value (avoids stale closure in handleWebViewMessage)
    if (hasRunningProcessRef.current) {
      if (__DEV__) console.log('[Terminal] Process already running, skipping init commands');
      return;
    }

    setTimeout(() => {
      const worktreePath = actualWorktreePath || feature.worktreePath;
      if (settings.autoLaunchAgent) {
        const agentCommand = settings.defaultAiAgent === 'claude' ? 'claude'
          : settings.defaultAiAgent === 'ollama' ? 'ollama run deepseek-coder'
          : settings.customAgentCommand || `echo "${t('terminal.error.no_agent')}"`;

        sendToTerminal(agentCommand + '\n');
      } else {
        sendToTerminal(`echo "🚀 NomadFlow - ${featureName}"\n`);
        setTimeout(() => sendToTerminal(`echo "📂 ${worktreePath}"\n`), 300);
      }
    }, 500);
  }, [actualWorktreePath, featureName, feature, settings, sendToTerminal, t]);

  // When the linked session is discovered, re-select the correct window in it.
  // tmux new-session does NOT inherit the base session's active window (defaults
  // to window 0), so we must explicitly select the desired window.
  // This mirrors what WebTerminal does on web.
  // Also runs sendInitCommands AFTER the correct window is selected (not before).
  const initCommandsSentRef = useRef(false);
  const linkedSessionHandledRef = useRef<string | null>(null);
  useEffect(() => {
    if (!server || !linkedSessionName || !repoPath || !featureName) return;
    // Guard: only re-run when linkedSessionName actually changes (not when
    // callback deps like sendInitCommands get a new identity).
    if (linkedSessionHandledRef.current === linkedSessionName) return;
    linkedSessionHandledRef.current = linkedSessionName;
    if (__DEV__) console.log('[Terminal] Re-selecting window in linked session:', linkedSessionName, 'feature:', featureName);
    switchToFeature(server, {
      repoPath,
      featureName,
      linkedSession: linkedSessionName,
    }).then((result) => {
      if (result.success && result.data) {
        hasRunningProcessRef.current = !!result.data.hasRunningProcess;
      }
      // Force tmux redraw via resize cycle
      triggerTerminalRedraw();
      // Now that the correct window is active, send init commands
      if (!initCommandsSentRef.current) {
        initCommandsSentRef.current = true;
        sendInitCommands();
      }
    }).catch((err) => {
      console.warn('[Terminal] Error re-selecting in linked session:', err);
      // Still send init commands on error (best effort)
      if (!initCommandsSentRef.current) {
        initCommandsSentRef.current = true;
        sendInitCommands();
      }
    });
  }, [linkedSessionName, server, repoPath, featureName, sendInitCommands, triggerTerminalRedraw]);

  const attemptReconnect = useCallback(() => {
    if (connectionState.reconnectAttempts >= settings.maxReconnectAttempts) {
      setConnectionState({ status: 'error', error: t('terminal.error.max_reconnect'), reconnectAttempts: connectionState.reconnectAttempts });
      return;
    }
    setTimeout(() => webViewRef.current?.reload(), settings.reconnectDelay);
  }, [connectionState.reconnectAttempts, settings, t]);

  const handleWebViewMessage = useCallback((event: WebViewMessageEvent) => {
    try {
      const message = JSON.parse(event.nativeEvent.data);
      switch (message.type) {
        case 'connected':
          setConnectionState({ status: 'connected', reconnectAttempts: 0 });
          // Init commands are sent from the linkedSessionName useEffect
          // (after the correct window is selected). Fallback: if linked_session
          // never arrives (e.g., discovery failure), send init after 2s.
          setTimeout(() => {
            if (!initCommandsSentRef.current) {
              if (__DEV__) console.log('[Terminal] Fallback: sending init commands (no linked session)');
              initCommandsSentRef.current = true;
              sendInitCommands();
            }
          }, 2000);
          break;
        case 'linked_session':
          linkedSessionRef.current = message.name;
          setLinkedSessionName(message.name);
          if (__DEV__) console.log('[Terminal] Linked session:', message.name);
          // Window selection is handled by the linkedSessionName useEffect
          break;
        case 'disconnected':
          setConnectionState((prev) => ({ status: 'disconnected', reconnectAttempts: prev.reconnectAttempts }));
          if (settings.autoReconnect) attemptReconnect();
          break;
        case 'error':
          setConnectionState({ status: 'error', error: message.error, reconnectAttempts: 0 });
          break;
        case 'scroll_state':
          // Touch scroll handler reports whether user has scrolled up from bottom
          break;
        case 'font_size':
          // Pinch-to-zoom handler reports new font size from WebView
          if (typeof message.size === 'number') {
            setCurrentFontSize(message.size);
            updateSettings({ fontSize: message.size });
          }
          break;
      }
    } catch (error) {
      console.error('Failed to parse WebView message:', error);
    }
  }, [settings.autoReconnect, updateSettings, sendInitCommands, attemptReconnect]);

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

  // Track keyboard visibility for ArrowKeysBar
  // iOS: keyboardWillShow fires before animation for smoother sync
  // Android: only keyboardDidShow/keyboardDidHide available
  useEffect(() => {
    const showEvent = Platform.OS === 'ios' ? 'keyboardWillShow' : 'keyboardDidShow';
    const hideEvent = Platform.OS === 'ios' ? 'keyboardWillHide' : 'keyboardDidHide';
    const showSub = Keyboard.addListener(showEvent, () => setKeyboardVisible(true));
    const hideSub = Keyboard.addListener(hideEvent, () => setKeyboardVisible(false));
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

  const handleSwitchSession = useCallback(
    async (sessionId: string) => {
      if (!server) return;
      const previousId = activeSessionId;
      setActiveSessionId(sessionId);
      const switchParams: Record<string, string> = { sessionId };
      if (linkedSessionRef.current) switchParams.linkedSession = linkedSessionRef.current;
      const result = await executeServerCommand(server, {
        action: 'select-session',
        params: switchParams,
      });
      if (!result.success) {
        console.warn('[Terminal] Error switching session:', result.error);
        setActiveSessionId(previousId);
        return;
      }
      // Force tmux to redraw by triggering a resize cycle in the WebView.
      // On web, xterm.js sees the new pty output immediately. On mobile, the ttyd
      // client inside the WebView may not repaint unless tmux sends a full redraw.
      triggerTerminalRedraw();
    },
    [server, activeSessionId, triggerTerminalRedraw]
  );

  const handleSwitchWorktree = useCallback(
    async (newWorktree: string) => {
      if (!server) return;

      // Update URL params so the UI (header, MiniBar) reflects the new worktree.
      // Note: expo-router may not re-trigger the switchToFeature useEffect when
      // replacing to the same pathname, so we call switchToFeature directly below.
      router.replace({
        pathname: '/terminal',
        params: { serverId: params.serverId, repoPath, featureName: newWorktree },
      } as any);

      // Ensure the 2-part tmux window exists and get worktree metadata
      try {
        const result = await switchToFeature(server, {
          repoPath,
          featureName: newWorktree,
          linkedSession: linkedSessionRef.current || undefined,
        });
        if (result.success && result.data) {
          setActualWorktreePath(result.data.worktreePath ?? null);
          hasRunningProcessRef.current = !!result.data.hasRunningProcess;
        }
      } catch (error) {
        console.warn('[Terminal] Error switching worktree:', error);
      }

      // If the target worktree has agent sessions, select the first session's
      // 3-part window (repo:worktree:agent-N) instead of the 2-part feature
      // window. The 2-part window is just a plain shell; the agent runs in the
      // 3-part window. This mirrors what the web does via selectSessionViaApi.
      const targetSessions = allSessions.filter((s) => s.worktree === newWorktree);
      if (targetSessions.length > 0) {
        const firstSession = targetSessions[0];
        setActiveSessionId(firstSession.sessionId);
        const switchParams: Record<string, string> = { sessionId: firstSession.sessionId };
        if (linkedSessionRef.current) switchParams.linkedSession = linkedSessionRef.current;
        await executeServerCommand(server, {
          action: 'select-session',
          params: switchParams,
        }).catch(() => {});
      } else {
        setActiveSessionId(null);
      }

      // Force tmux redraw via resize cycle
      triggerTerminalRedraw();
    },
    [server, router, params.serverId, repoPath, allSessions, triggerTerminalRedraw]
  );

  const handleCreateSession = useCallback(
    async (agentType: string) => {
      if (!server) return;
      const worktreePath = actualWorktreePath ?? feature.worktreePath;
      await executeServerCommand(server, {
        action: 'create-session',
        params: { worktreePath, agentType },
      });
    },
    [server, actualWorktreePath, feature.worktreePath]
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
  // Build the tmux window name (repo_basename:featureName) so the server
  // can select the correct window in the linked session before the bridge starts.
  const tmuxWindowName = useMemo(() => {
    if (!repoPath || !featureName) return undefined;
    const repoBasename = repoPath.split('/').filter(Boolean).pop() || repoPath;
    return `${repoBasename}:${featureName}`;
  }, [repoPath, featureName]);

  const injectedJS = useMemo(
    () => buildInjectedJS(apiUrl, server?.authToken, tmuxWindowName),
    [apiUrl, server?.authToken, tmuxWindowName]
  );

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
  const terminalUrl = buildTerminalUrl(server);
  const basicAuthCredential = server.authToken
    ? { username: 'nomadflow', password: server.authToken }
    : undefined;

  const statusColor = connectionState.status === 'connected' ? 'text-success' : connectionState.status === 'error' || connectionState.status === 'disconnected' ? 'text-destructive' : 'text-warning';

  if (isPreparingTerminal) {
    return (
      <View className="flex-1 bg-black" style={{ paddingTop: insets.top }}>
        <Stack.Screen options={{ headerShown: false }} />
        <View className="flex-1 items-center justify-center">
          <ActivityIndicator size="large" color="hsl(250, 85%, 65%)" />
          <Text className="mt-4 text-muted-foreground">{t('terminal.preparing')}</Text>
          <Text className="mt-2 text-sm text-muted-foreground">{featureName}</Text>
        </View>
      </View>
    );
  }

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
            <Text className={`text-xs ${statusColor}`}>{t(`terminal.status.${connectionState.status}`)}</Text>
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
            console.warn('[Terminal] HTTP error:', statusCode);
            if (statusCode === 401) {
              setConnectionState({
                status: 'error',
                error: t('terminal.error.auth_failed'),
                reconnectAttempts: 0,
              });
            }
          }}
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
          injectedJavaScriptBeforeContentLoaded={injectedJS}
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

        {(connectionState.status === 'error' || connectionState.status === 'disconnected') && (
          <View className="absolute inset-0 items-center justify-center bg-black/90 p-8">
            <Icon as={WifiOffIcon} className="mb-4 text-muted-foreground" size={64} />
            <Text className="mb-2 text-xl font-semibold">
              {connectionState.status === 'error' ? t('terminal.error.connection_title') : t('terminal.status.disconnected')}
            </Text>
            <Text className="mb-6 text-center text-muted-foreground">
              {connectionState.error || t('terminal.error.connection_lost')}
            </Text>
            <Button onPress={() => { setConnectionState({ status: 'connecting', reconnectAttempts: 0 }); webViewRef.current?.reload(); }}>
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

      {/* Arrow Keys Bar — only on mobile, visible when keyboard is open */}
      {Platform.OS !== 'web' && (
        <ArrowKeysBar onSendKey={sendToTerminal} visible={keyboardVisible} />
      )}

      {/* MiniBar — inside KeyboardAvoidingView so Android keyboard doesn't overlap */}
      {Platform.OS !== 'web' && (
        <MiniBar
          sessions={worktreeSessions}
          allSessions={allSessions}
          activeSessionId={activeSessionId}
          worktreeName={featureName}
          onSwitchSession={handleSwitchSession}
          onSwitchWorktree={handleSwitchWorktree}
          onCreateSession={handleCreateSession}
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

        <Text className="mb-2 text-xs text-muted-foreground">{t('terminal.shortcuts.tmux_title')}</Text>
        <View className="mb-4 flex-row flex-wrap">
          {TMUX_SHORTCUTS.map((s) => (
            <Pressable key={s.key} onPress={() => sendTmuxKey(s.key)} className="w-1/4 items-center p-2">
              <View className="mb-1 h-10 w-10 items-center justify-center rounded-lg bg-background">
                <Icon as={s.icon} className="text-primary" size={18} />
              </View>
              <Text className="text-xs font-medium">{t(s.labelKey)}</Text>
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

      {/* Swipe Drawers — mobile only */}
      {Platform.OS !== 'web' && (
        <>
          <DrawerPanel side="left" isOpen={leftDrawerOpen} onClose={closeDrawers}>
            <WorktreeDrawerContent
              allSessions={allSessions}
              activeSessionId={activeSessionId}
              currentWorktree={featureName}
              features={allFeatures}
              onSwitchSession={handleSwitchSession}
              onSwitchWorktree={handleSwitchWorktree}
              onClose={closeDrawers}
            />
          </DrawerPanel>
          <DrawerPanel side="right" isOpen={rightDrawerOpen} onClose={closeDrawers}>
            <DiffDrawerContent
              server={server}
              worktreePath={actualWorktreePath || feature.worktreePath}
              isOpen={rightDrawerOpen}
              onClose={closeDrawers}
              onFileClick={setSelectedDiffFile}
            />
          </DrawerPanel>
        </>
      )}

      {/* Diff file overlay — above drawers */}
      {Platform.OS !== 'web' && selectedDiffFile && (
        <MobileDiffView
          server={server}
          worktreePath={actualWorktreePath || feature.worktreePath}
          filePath={selectedDiffFile}
          onClose={() => setSelectedDiffFile(null)}
        />
      )}
    </View>
  );
}
