#!/usr/bin/env node
/**
 * build-terminal-html.js — Generates assets/terminal.html by inlining xterm.js
 * dependencies from node_modules and our custom WS handler / touch handlers.
 *
 * Usage: node scripts/build-terminal-html.js
 * Output: assets/terminal.html + assets/terminal-html.ts
 */

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const OUT_HTML = path.join(ROOT, 'assets', 'terminal.html');
const OUT_TS = path.join(ROOT, 'assets', 'terminal-html.ts');

// Read xterm dependencies from node_modules
const xtermJS = fs.readFileSync(
  path.join(ROOT, 'node_modules/@xterm/xterm/lib/xterm.js'),
  'utf-8'
);
const xtermCSS = fs.readFileSync(
  path.join(ROOT, 'node_modules/@xterm/xterm/css/xterm.css'),
  'utf-8'
);
const fitAddonJS = fs.readFileSync(
  path.join(ROOT, 'node_modules/@xterm/addon-fit/lib/addon-fit.js'),
  'utf-8'
);

const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<title>NomadFlow Terminal</title>
<style>
html, body { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #0f0f17; }
#terminal { width: 100%; height: 100%; }
/* Hide xterm helper textarea caret */
.xterm-helper-textarea { caret-color: transparent !important; opacity: 0 !important; }
</style>
<style>${xtermCSS}</style>
</head>
<body>
<div id="terminal"></div>

<!-- xterm.js core -->
<script>${xtermJS}</script>
<!-- xterm addon-fit -->
<script>${fitAddonJS}</script>

<script>
// ============================================================
// NomadFlow Terminal — Custom WS Handler + Touch Handlers
// ============================================================

(function() {
  'use strict';

  // --- Configuration (injected by React Native before content loads) ---
  var config = window.__NOMADFLOW_CONFIG__ || {};

  // --- Frame type constants (matching protocol.rs / terminal-ws.ts) ---
  var PTY_DATA = 0x01;
  var RESIZE = 0x02;
  var CONTROL = 0x03;
  var BUFFER_SNAPSHOT = 0x04;
  var PING = 0x05;
  var CONTROL_PANE_ID = 0x0000;

  // --- State ---
  var ws = null;
  var term = null;
  var fitAddon = null;
  var activePaneId = null;
  var paneList = [];
  var reconnectAttempts = 0;
  var MAX_RECONNECT = 10;
  var reconnectTimer = null;
  var connected = false;
  var termReady = false;
  var pendingCreate = false;
  var pendingSwitchPaneId = null;
  var pendingSwitchLabel = '';

  // --- Helpers ---
  var encoder = new TextEncoder();
  var decoder = new TextDecoder();

  function postToRN(msg) {
    if (typeof window.ReactNativeWebView !== 'undefined') {
      window.ReactNativeWebView.postMessage(JSON.stringify(msg));
    }
  }

  // --- Frame encoding / decoding ---

  function encodeFrame(type, paneId, payload) {
    var frame = new Uint8Array(3 + payload.length);
    frame[0] = type;
    frame[1] = (paneId >> 8) & 0xFF;
    frame[2] = paneId & 0xFF;
    frame.set(payload, 3);
    return frame.buffer;
  }

  function decodeFrame(data) {
    var bytes = new Uint8Array(data);
    if (bytes.length < 3) return null;
    return {
      type: bytes[0],
      paneId: (bytes[1] << 8) | bytes[2],
      payload: bytes.slice(3)
    };
  }

  function encodeResize(cols, rows) {
    var payload = new Uint8Array(4);
    payload[0] = (cols >> 8) & 0xFF;
    payload[1] = cols & 0xFF;
    payload[2] = (rows >> 8) & 0xFF;
    payload[3] = rows & 0xFF;
    return payload;
  }

  // --- Send functions ---

  function sendInput(paneId, data) {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    var encoded = encoder.encode(data);
    ws.send(encodeFrame(PTY_DATA, paneId, encoded));
  }

  function sendResize(paneId, cols, rows) {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(encodeFrame(RESIZE, paneId, encodeResize(cols, rows)));
  }

  function sendControl(msg) {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    var json = encoder.encode(JSON.stringify(msg));
    ws.send(encodeFrame(CONTROL, CONTROL_PANE_ID, json));
  }

  // --- Pane subscription ---

  function subscribeToPaneById(paneId) {
    activePaneId = paneId;
    sendControl({ type: 'subscribe', paneIds: [paneId] });
  }

  function switchPane(fromPaneId, toPaneId) {
    if (fromPaneId != null && fromPaneId === toPaneId) return;
    if (fromPaneId != null) {
      sendControl({ type: 'unsubscribe', paneIds: [fromPaneId] });
    }
    activePaneId = toPaneId;
    sendControl({ type: 'subscribe', paneIds: [toPaneId] });
  }

  function createPane(req) {
    pendingCreate = true;
    sendControl(Object.assign({ type: 'create' }, req));
  }

  // --- WS message handler ---

  function handleWsMessage(event) {
    if (!(event.data instanceof ArrayBuffer)) return;
    var frame = decodeFrame(event.data);
    if (!frame) return;

    switch (frame.type) {
      case PTY_DATA:
        if (frame.paneId === activePaneId && term) {
          term.write(frame.payload);
        }
        break;

      case BUFFER_SNAPSHOT:
        if (frame.paneId === activePaneId && term) {
          term.clear();
          term.write(frame.payload);
        }
        if (frame.paneId === pendingSwitchPaneId) {
          postToRN({ type: 'paneSwitched', paneId: pendingSwitchPaneId, label: pendingSwitchLabel });
          pendingSwitchPaneId = null;
          pendingSwitchLabel = '';
        }
        break;

      case CONTROL:
        try {
          var msg = JSON.parse(decoder.decode(frame.payload));
          handleControlMessage(msg);
        } catch (e) {
          console.error('[Terminal] Control decode error:', e);
        }
        break;

      case PING:
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(encodeFrame(PING, CONTROL_PANE_ID, new Uint8Array(0)));
        }
        break;
    }
  }

  function handleControlMessage(msg) {
    switch (msg.type) {
      case 'paneList':
        paneList = msg.panes || [];
        postToRN({ type: 'paneList', panes: paneList });
        if (activePaneId == null && termReady) {
          autoSubscribe();
        }
        break;

      case 'paneCreated':
        var exists = false;
        for (var i = 0; i < paneList.length; i++) {
          if (paneList[i].id === msg.id) { exists = true; break; }
        }
        if (!exists) paneList.push(msg);
        postToRN({ type: 'paneList', panes: paneList });
        var wasPendingCreate = pendingCreate;
        if (activePaneId == null || pendingCreate) {
          pendingCreate = false;
          switchPane(activePaneId, msg.id);
          pendingSwitchPaneId = msg.id;
          pendingSwitchLabel = msg.label;
        }
        if (wasPendingCreate && config.autoLaunchAgent && config.agentType !== 'shell') {
          (function(pId) {
            setTimeout(function() {
              var cmd = config.defaultAiAgent === 'claude' ? 'claude'
                      : config.defaultAiAgent === 'ollama' ? 'ollama run deepseek-coder'
                      : config.customAgentCommand || '';
              if (cmd && pId != null) sendInput(pId, cmd + '\\n');
            }, 500);
          })(msg.id);
        }
        break;

      case 'paneDestroyed':
        paneList = paneList.filter(function(p) { return p.id !== msg.paneId; });
        postToRN({ type: 'paneDestroyed', paneId: msg.paneId });
        postToRN({ type: 'paneList', panes: paneList });
        if (msg.paneId === activePaneId) {
          activePaneId = null;
          if (term) {
            term.write('\\r\\n\\x1b[33m[Terminal session ended]\\x1b[0m\\r\\n');
          }
        }
        break;

      case 'error':
        console.error('[Terminal] Server error:', msg.message);
        postToRN({ type: 'error', message: msg.message });
        break;
    }
  }

  function autoSubscribe() {
    if (config.repo && config.worktree) {
      for (var i = 0; i < paneList.length; i++) {
        var p = paneList[i];
        if (p.repo === config.repo && p.worktree === config.worktree) {
          subscribeToPaneById(p.id);
          pendingSwitchPaneId = p.id;
          pendingSwitchLabel = p.label;
          return;
        }
      }
    }
    if (config.repo || config.worktree) {
      var dims = fitAddon ? fitAddon.proposeDimensions() : null;
      createPane({
        repo: config.repo || '',
        worktree: config.worktree || '',
        agentType: config.agentType || 'shell',
        cwd: config.cwd || '',
        cols: dims ? dims.cols : 80,
        rows: dims ? dims.rows : 24
      });
    } else if (paneList.length > 0) {
      subscribeToPaneById(paneList[0].id);
      pendingSwitchPaneId = paneList[0].id;
      pendingSwitchLabel = paneList[0].label;
    }
  }

  function connect() {
    var wsUrl = config.wsUrl;
    var token = config.token;
    if (!wsUrl) return;
    if (token) {
      wsUrl += (wsUrl.indexOf('?') >= 0 ? '&' : '?') + 'token=' + encodeURIComponent(token);
    }
    ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    ws.onopen = function() {
      connected = true;
      reconnectAttempts = 0;
      if (activePaneId != null) {
        sendControl({ type: 'subscribe', paneIds: [activePaneId] });
      }
      postToRN({ type: 'connected' });
    };
    ws.onmessage = handleWsMessage;
    ws.onclose = function() {
      connected = false;
      postToRN({ type: 'disconnected' });
      scheduleReconnect();
    };
    ws.onerror = function() {};
  }

  function scheduleReconnect() {
    if (reconnectAttempts >= MAX_RECONNECT) return;
    var delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 8000);
    reconnectAttempts++;
    reconnectTimer = setTimeout(connect, delay);
  }

  function disconnect() {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    reconnectAttempts = MAX_RECONNECT;
    if (ws) {
      ws.onclose = null;
      ws.close();
      ws = null;
    }
    connected = false;
  }

  function initTerminal() {
    term = new Terminal({
      cursorBlink: true,
      fontSize: config.fontSize || 14,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: { background: '#0f0f17', foreground: '#e4e4e7', cursor: '#a78bfa' },
      allowProposedApi: true
    });
    fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);
    term.open(document.getElementById('terminal'));
    fitAddon.fit();
    term.onData(function(data) {
      if (activePaneId != null) {
        sendInput(activePaneId, data);
      }
    });
    term.onResize(function(size) {
      if (activePaneId != null) {
        sendResize(activePaneId, size.cols, size.rows);
      }
      postToRN({ type: 'resized', cols: size.cols, rows: size.rows });
    });
    window.addEventListener('resize', function() {
      if (fitAddon) fitAddon.fit();
    });
    termReady = true;
    if (paneList.length > 0 && activePaneId == null) {
      autoSubscribe();
    }
  }

  // === TouchScrollHandler ===
  (function() {
    var DEFAULT_LINE_HEIGHT = 20;
    var MOVE_THRESHOLD = 10;
    var DECAY_FACTOR = 0.95;
    var MIN_VELOCITY = 0.5;
    var VELOCITY_SAMPLES = 3;
    var VELOCITY_THRESHOLD = 0.01;

    function getLineHeight() {
      try {
        if (term && term._core && term._core._renderService) {
          var dims = term._core._renderService.dimensions;
          return (dims.css && dims.css.cell && dims.css.cell.height) || dims.device.cell.height || DEFAULT_LINE_HEIGHT;
        }
      } catch(e) {}
      return DEFAULT_LINE_HEIGHT;
    }

    var state = 'IDLE';
    var startX = 0, startY = 0, lastY = 0;
    var touchId = null;
    var directionLocked = false, isVertical = false;
    var momentumId = null;
    var userScrolledUp = false;
    var inTmuxCopyMode = false;
    var velocitySamples = [];
    var accumulatedDelta = 0;

    function enterTmuxCopyMode() {
      if (!inTmuxCopyMode && activePaneId != null) {
        sendInput(activePaneId, '\\x02[');
        inTmuxCopyMode = true;
      }
    }

    function exitTmuxCopyMode() {
      if (inTmuxCopyMode && activePaneId != null) {
        sendInput(activePaneId, '\\x1b');
        inTmuxCopyMode = false;
      }
    }

    function addVelocitySample(y, time) {
      velocitySamples.push({ y: y, time: time });
      if (velocitySamples.length > VELOCITY_SAMPLES) velocitySamples.shift();
    }

    function calculateVelocity() {
      if (velocitySamples.length < 2) return 0;
      var first = velocitySamples[0];
      var last = velocitySamples[velocitySamples.length - 1];
      var dt = last.time - first.time;
      if (dt <= 0) return 0;
      return (last.y - first.y) / dt;
    }

    function cancelMomentum() {
      if (momentumId) {
        cancelAnimationFrame(momentumId);
        momentumId = null;
      }
    }

    function startMomentum(velocityPxMs) {
      if (!term) return;
      var isAlternate = term.buffer.active.type !== 'normal';
      if (isAlternate && !inTmuxCopyMode) return;
      var frameTime = 16;
      var velocity = velocityPxMs * frameTime;
      var lineHeight = getLineHeight();
      var momentumAccumulator = 0;
      function tick() {
        velocity *= DECAY_FACTOR;
        momentumAccumulator += velocity;
        var linesToScroll = Math.trunc(momentumAccumulator / lineHeight);
        if (linesToScroll !== 0) {
          if (inTmuxCopyMode && activePaneId != null) {
            var count = Math.min(10, Math.abs(linesToScroll));
            var seq = velocity > 0 ? '\\x1b[A' : '\\x1b[B';
            for (var j = 0; j < count; j++) sendInput(activePaneId, seq);
          } else {
            term.scrollLines(-linesToScroll);
            updateScrollState();
          }
          momentumAccumulator -= linesToScroll * lineHeight;
        }
        if (Math.abs(velocity) < MIN_VELOCITY) {
          momentumId = null;
          return;
        }
        momentumId = requestAnimationFrame(tick);
      }
      momentumId = requestAnimationFrame(tick);
    }

    function updateScrollState() {
      if (!term) return;
      var buf = term.buffer.active;
      var atBottom = buf.viewportY >= buf.baseY;
      if (atBottom && userScrolledUp) {
        userScrolledUp = false;
        postToRN({ type: 'scroll_state', atBottom: true });
      } else if (!atBottom && !userScrolledUp) {
        userScrolledUp = true;
        postToRN({ type: 'scroll_state', atBottom: false });
      }
    }

    function resetState() {
      state = 'IDLE'; touchId = null;
      directionLocked = false; isVertical = false;
      velocitySamples = []; accumulatedDelta = 0;
    }

    document.addEventListener('touchstart', function(e) {
      if (e.touches.length > 1) {
        cancelMomentum(); exitTmuxCopyMode(); resetState(); return;
      }
      cancelMomentum();
      var touch = e.touches[0];
      touchId = touch.identifier;
      startX = touch.clientX; startY = touch.clientY; lastY = touch.clientY;
      directionLocked = false; isVertical = false;
      velocitySamples = []; accumulatedDelta = 0;
      state = 'TRACKING';
    }, { passive: true });

    document.addEventListener('touchmove', function(e) {
      if (state !== 'TRACKING' && state !== 'SCROLLING') return;
      if (e.touches.length > 1) { resetState(); return; }
      var touch = null;
      for (var i = 0; i < e.touches.length; i++) {
        if (e.touches[i].identifier === touchId) { touch = e.touches[i]; break; }
      }
      if (!touch) return;
      if (state === 'TRACKING') {
        var absDx = Math.abs(touch.clientX - startX);
        var absDy = Math.abs(touch.clientY - startY);
        if (!directionLocked && (absDy > MOVE_THRESHOLD || absDx > MOVE_THRESHOLD)) {
          directionLocked = true; isVertical = absDy >= absDx;
        }
        if (!directionLocked || !isVertical) return;
        state = 'SCROLLING'; lastY = touch.clientY;
      }
      e.preventDefault();
      var moveDelta = touch.clientY - lastY;
      lastY = touch.clientY;
      addVelocitySample(touch.clientY, e.timeStamp);
      if (!term) return;
      accumulatedDelta += moveDelta;
      var currentLineHeight = getLineHeight();
      var linesToScroll = Math.trunc(accumulatedDelta / currentLineHeight);
      if (linesToScroll !== 0) {
        if (term.buffer.active.type === 'alternate') {
          enterTmuxCopyMode();
          var count = Math.min(10, Math.abs(linesToScroll));
          var seq = linesToScroll > 0 ? '\\x1b[A' : '\\x1b[B';
          if (activePaneId != null) {
            for (var j = 0; j < count; j++) sendInput(activePaneId, seq);
          }
        } else {
          term.scrollLines(-linesToScroll);
          updateScrollState();
        }
        accumulatedDelta -= linesToScroll * currentLineHeight;
      }
    }, { passive: false });

    document.addEventListener('touchend', function(e) {
      if (state === 'SCROLLING') {
        var velocity = calculateVelocity();
        var allowMomentum = term && (term.buffer.active.type === 'normal' || inTmuxCopyMode);
        if (Math.abs(velocity) > VELOCITY_THRESHOLD && allowMomentum) {
          state = 'MOMENTUM'; startMomentum(velocity);
        }
      } else if (state === 'TRACKING' && inTmuxCopyMode) {
        exitTmuxCopyMode();
      }
      resetState();
    }, { passive: true });

    document.addEventListener('touchcancel', function() {
      cancelMomentum(); resetState();
    }, { passive: true });

    window.addEventListener('resize', function() {
      cancelMomentum(); exitTmuxCopyMode();
    });
  })();

  // === PinchZoomHandler ===
  (function() {
    var PINCH_MIN_FONT = 8, PINCH_MAX_FONT = 56;
    var pinchInitialDistance = 0, pinchInitialFontSize = 0, pinchActive = false;
    function getPinchDistance(touches) {
      var dx = touches[0].clientX - touches[1].clientX;
      var dy = touches[0].clientY - touches[1].clientY;
      return Math.sqrt(dx * dx + dy * dy);
    }
    document.addEventListener('touchstart', function(e) {
      if (e.touches.length === 2 && term) {
        pinchActive = true;
        pinchInitialDistance = getPinchDistance(e.touches);
        pinchInitialFontSize = term.options.fontSize || PINCH_MIN_FONT;
      }
    }, { passive: true });
    document.addEventListener('touchmove', function(e) {
      if (!pinchActive || e.touches.length !== 2 || !term || pinchInitialDistance === 0) return;
      e.preventDefault();
      var currentDistance = getPinchDistance(e.touches);
      var scale = currentDistance / pinchInitialDistance;
      var newSize = Math.max(PINCH_MIN_FONT, Math.min(PINCH_MAX_FONT, Math.round(pinchInitialFontSize * scale)));
      if (newSize !== term.options.fontSize) {
        term.options.fontSize = newSize; if (fitAddon) fitAddon.fit();
      }
    }, { passive: false });
    document.addEventListener('touchend', function(e) {
      if (e.touches.length < 2 && pinchActive) {
        pinchActive = false; pinchInitialDistance = 0;
        if (term) {
          postToRN({ type: 'font_size', fontSize: term.options.fontSize });
          if (activePaneId != null && fitAddon) {
            var dims = fitAddon.proposeDimensions();
            if (dims) sendResize(activePaneId, dims.cols, dims.rows);
          }
        }
      }
    }, { passive: true });
  })();

  // === PostMessage bridge (RN → HTML) ===
  function handleRNMessage(event) {
    var data;
    try { data = typeof event.data === 'string' ? JSON.parse(event.data) : event.data; } catch (e) { return; }
    switch (data.type) {
      case 'connect':
        if (data.wsUrl) config.wsUrl = data.wsUrl;
        if (data.token) config.token = data.token;
        if (data.repo) config.repo = data.repo;
        if (data.worktree) config.worktree = data.worktree;
        if (data.agentType) config.agentType = data.agentType;
        if (data.fontSize && term) {
          term.options.fontSize = data.fontSize; if (fitAddon) fitAddon.fit();
        }
        connect();
        break;
      case 'switchPane':
        if (typeof data.paneId === 'number') {
          switchPane(activePaneId, data.paneId);
          var targetPane = paneList.find(function(p) { return p.id === data.paneId; });
          pendingSwitchPaneId = data.paneId;
          pendingSwitchLabel = targetPane ? targetPane.label : '';
        }
        break;
      case 'sendInput':
        if (activePaneId != null && data.data) sendInput(activePaneId, data.data);
        break;
      case 'resize':
        if (fitAddon) fitAddon.fit();
        break;
      case 'setFontSize':
        if (typeof data.fontSize === 'number' && term) {
          term.options.fontSize = data.fontSize; if (fitAddon) fitAddon.fit();
        }
        break;
      case 'createPane':
        if (data.request) createPane(data.request);
        break;
      case 'destroyPane':
        if (typeof data.paneId === 'number') {
          sendControl({ type: 'destroy', paneId: data.paneId });
          if (data.paneId === activePaneId) {
            activePaneId = null;
            if (term) term.write('\\r\\n\\x1b[33m[Terminal session ended]\\x1b[0m\\r\\n');
          }
        }
        break;
      case 'disconnect': disconnect(); break;
      case 'blur':
        if (document.activeElement) document.activeElement.blur();
        break;
    }
  }
  window.addEventListener('message', handleRNMessage);
  document.addEventListener('message', handleRNMessage);

  function boot() {
    initTerminal();
    if (config.wsUrl) connect();
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot);
  } else {
    boot();
  }
})();
</script>
</body>
</html>`;

// Write raw HTML
fs.writeFileSync(OUT_HTML, html, 'utf-8');
const htmlSizeStr = (fs.statSync(OUT_HTML).size / 1024).toFixed(1);
process.stdout.write("Generated " + OUT_HTML + " (" + htmlSizeStr + " KB)\n");

// Write TypeScript module
const tsContent = "// AUTO-GENERATED by scripts/build-terminal-html.js — DO NOT EDIT\n" +
  "// Re-generate with: node scripts/build-terminal-html.js\n" +
  "export default " + JSON.stringify(html) + ";\n";
fs.writeFileSync(OUT_TS, tsContent, 'utf-8');
const tsSizeStr = (fs.statSync(OUT_TS).size / 1024).toFixed(1);
process.stdout.write("Generated " + OUT_TS + " (" + tsSizeStr + " KB)\n");
