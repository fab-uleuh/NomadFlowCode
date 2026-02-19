import '@/global.css';
import '@/lib/i18n';

import { StorageProvider } from '@/lib/context/storage-context';
import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronRight, Moon, Sun, Settings, Sparkles } from 'lucide-react-native';
import { Sidebar } from '@/components/web/Sidebar';
import { SplitTerminal } from '@/components/web/SplitTerminal';
import type { SplitTerminalHandle } from '@/components/web/SplitTerminal';
import { AgentTabBar } from '@/components/web/AgentTabBar';
import { AddServerDialog } from '@/components/web/AddServerDialog';
import { SettingsDialog } from '@/components/web/SettingsDialog';
import { CloneRepoDialog } from '@/components/web/CloneRepoDialog';
import { CreateFeatureDialog } from '@/components/web/CreateFeatureDialog';
import { CreateSessionDialog } from '@/components/web/CreateSessionDialog';
import { CommandPalette } from '@/components/web/CommandPalette';
import { DiffPanel } from '@/components/web/DiffPanel';
import { FileDiffView } from '@/components/web/FileDiffView';
import { FileContentView } from '@/components/web/FileContentView';
import type { Server, Repository, Feature } from '@shared';
import type { ConnectionState } from '@/lib/types';
import type { SessionWithState } from '@/lib/types/session';

type MainPanelMode = 'terminal' | 'diff' | 'file-content';

/** Shallow-compare session lists to avoid unnecessary re-renders from polling. */
function sessionsEqual(a: SessionWithState[], b: SessionWithState[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i].sessionId !== b[i].sessionId || a[i].agentState !== b[i].agentState) return false;
  }
  return true;
}

function WebApp() {
  const { t } = useTranslation();


  const [colorScheme, setColorScheme] = useState<'light' | 'dark'>(() => {
    if (typeof window !== 'undefined') {
      const saved = localStorage.getItem('@nomadflow_color_scheme');
      if (saved === 'light' || saved === 'dark') return saved;
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return 'dark';
  });

  useEffect(() => {
    document.documentElement.classList.toggle('dark', colorScheme === 'dark');
    localStorage.setItem('@nomadflow_color_scheme', colorScheme);
  }, [colorScheme]);

  const toggleTheme = useCallback(() => {
    setColorScheme((prev) => (prev === 'dark' ? 'light' : 'dark'));
  }, []);

  // Selection state
  const [selectedServer, setSelectedServer] = useState<Server | null>(null);
  const [selectedRepo, setSelectedRepo] = useState<Repository | null>(null);
  const [selectedFeature, setSelectedFeature] = useState<Feature | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionWithState | null>(null);
  const [currentWorktreePath, setCurrentWorktreePath] = useState('');

  // Refs for stable callbacks (avoid re-creating handlers on every selection change)
  const selectedSessionRef = useRef<SessionWithState | null>(null);
  const selectedServerRef = useRef<Server | null>(null);
  const currentWorktreePathRef = useRef('');
  useEffect(() => { selectedSessionRef.current = selectedSession; }, [selectedSession]);
  useEffect(() => { selectedServerRef.current = selectedServer; }, [selectedServer]);
  useEffect(() => { currentWorktreePathRef.current = currentWorktreePath; }, [currentWorktreePath]);

  // All sessions from polling (keyed by server id)
  const [allSessionsByServer, setAllSessionsByServer] = useState<
    Map<string, SessionWithState[]>
  >(new Map());

  // Terminal ref for imperative split pane control
  const splitTerminalRef = useRef<SplitTerminalHandle>(null);

  // Dialog state
  const [showAddServer, setShowAddServer] = useState(false);
  const [editingServer, setEditingServer] = useState<Server | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [showCloneRepo, setShowCloneRepo] = useState(false);
  const [cloneRepoServer, setCloneRepoServer] = useState<Server | null>(null);
  const [showCreateFeature, setShowCreateFeature] = useState(false);
  const [createFeatureServer, setCreateFeatureServer] = useState<Server | null>(null);
  const [createFeatureRepoPath, setCreateFeatureRepoPath] = useState('');
  const [showCreateSession, setShowCreateSession] = useState(false);
  const [createSessionServer, setCreateSessionServer] = useState<Server | null>(null);
  const [createSessionWorktreePath, setCreateSessionWorktreePath] = useState('');
  const [showCommandPalette, setShowCommandPalette] = useState(false);
  const [showDiffPanel, setShowDiffPanel] = useState(false);
  const [mainPanelMode, setMainPanelMode] = useState<MainPanelMode>('terminal');
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [connectionState, setConnectionState] = useState<ConnectionState | null>(null);

  const handleSelectFeature = useCallback(
    (server: Server, repo: Repository, feature: Feature) => {
      setSelectedServer(server);
      setSelectedRepo(repo);
      setSelectedFeature(feature);
      setSelectedSession(null);
      setCurrentWorktreePath(feature.worktreePath);
    },
    []
  );

  const handleSelectSession = useCallback(
    (server: Server, session: SessionWithState, worktreePath: string) => {
      setCurrentWorktreePath(worktreePath);

      const prev = selectedSessionRef.current;
      const prevServer = selectedServerRef.current;
      const sameWorktree =
        prev &&
        prevServer?.id === server.id &&
        session.repo === prev.repo &&
        session.worktree === prev.worktree;

      if (sameWorktree) {
        // In-place switch — only the focused pane switches session
        setSelectedSession(session);
        splitTerminalRef.current?.selectSessionInFocusedPane(session.sessionId);
      } else {
        // Different worktree or server — full remount
        setSelectedServer(server);
        setSelectedRepo(null);
        setSelectedSession(session);
        setSelectedFeature(null);
      }
    },
    []
  );

  const handleCreateSession = useCallback((server: Server, worktreePath: string) => {
    setCreateSessionServer(server);
    setCreateSessionWorktreePath(worktreePath);
    setShowCreateSession(true);
  }, []);

  const handleSessionsUpdate = useCallback(
    (server: Server, sessions: SessionWithState[]) => {
      setAllSessionsByServer((prev) => {
        const existing = prev.get(server.id);
        if (existing && sessionsEqual(existing, sessions)) {
          return prev;
        }
        const next = new Map(prev);
        next.set(server.id, sessions);
        return next;
      });
    },
    []
  );

  const handleEditServer = useCallback((server: Server) => {
    setEditingServer(server);
    setShowAddServer(true);
  }, []);

  const handleCloneRepo = useCallback((server: Server) => {
    setCloneRepoServer(server);
    setShowCloneRepo(true);
  }, []);

  const handleCreateFeature = useCallback((server: Server, repoPath: string) => {
    setCreateFeatureServer(server);
    setCreateFeatureRepoPath(repoPath);
    setShowCreateFeature(true);
  }, []);

  const handleFeatureCreated = useCallback(
    (featureName: string, worktreePath: string, branch: string) => {
      if (createFeatureServer && createFeatureRepoPath) {
        const newFeature: Feature = {
          name: featureName,
          worktreePath,
          branch,
          isActive: true,
          createdAt: Date.now(),
        };
        const repo: Repository = {
          name: createFeatureRepoPath.split('/').pop() || '',
          path: createFeatureRepoPath,
          branch,
        };
        handleSelectFeature(createFeatureServer, repo, newFeature);
      }
      setShowCreateFeature(false);
    },
    [createFeatureServer, createFeatureRepoPath, handleSelectFeature]
  );

  // Compute worktree sessions for the tab bar
  const worktreeSessions = useMemo(() => {
    if (!selectedSession || !selectedServer) return [];
    const serverSessions = allSessionsByServer.get(selectedServer.id) ?? [];
    return serverSessions.filter(
      (s) => s.repo === selectedSession.repo && s.worktree === selectedSession.worktree
    );
  }, [selectedSession, selectedServer, allSessionsByServer]);

  // Shell sessions for the current worktree (used by SplitTerminal for stateless restoration)
  const shellSessions = useMemo(() => {
    if (!selectedServer) return [];
    const serverSessions = allSessionsByServer.get(selectedServer.id) ?? [];
    const repo = selectedRepo?.name || selectedSession?.repo || '';
    const worktree = selectedFeature?.name || selectedSession?.worktree || '';
    return serverSessions.filter(
      (s) => s.repo === repo && s.worktree === worktree && s.agentType === 'shell'
    );
  }, [selectedServer, selectedRepo, selectedFeature, selectedSession, allSessionsByServer]);

  // Handle tab bar "+" button
  const handleTabBarCreateSession = useCallback(() => {
    const server = selectedServerRef.current;
    const session = selectedSessionRef.current;
    if (server && session) {
      handleCreateSession(server, session.worktree);
    }
  }, [handleCreateSession]);

  // Handle tab bar session select
  const handleTabBarSelectSession = useCallback(
    (session: SessionWithState) => {
      const server = selectedServerRef.current;
      if (server) {
        handleSelectSession(server, session, currentWorktreePathRef.current);
      }
    },
    [handleSelectSession]
  );

  // Collapse split panes when all sessions close (M3: task 5.3 fix)
  useEffect(() => {
    if (selectedSession && worktreeSessions.length === 0) {
      splitTerminalRef.current?.resetToSinglePane();
    }
  }, [worktreeSessions.length, selectedSession]);

  // Toggle diff panel callback (shared between Cmd+B and ShortcutBar)
  const toggleDiffPanel = useCallback(() => {
    setShowDiffPanel((prev) => !prev);
  }, []);

  // File click from diff panel → show diff view
  const handleFileClick = useCallback((filePath: string) => {
    setMainPanelMode('diff');
    setSelectedFilePath(filePath);
  }, []);

  // Return to terminal from diff/file-content view
  const handleBackToTerminal = useCallback(() => {
    setMainPanelMode('terminal');
    setSelectedFilePath(null);
  }, []);

  // Cmd+K / Ctrl+K → toggle command palette, Cmd+B / Ctrl+B → toggle diff panel (skip if a dialog is open)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (showAddServer || showSettings || showCloneRepo || showCreateFeature || showCreateSession || showCommandPalette) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setShowCommandPalette((prev) => !prev);
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        toggleDiffPanel();
      }
      // Esc → return to terminal when in diff/file-content mode (not when terminal is active — terminal needs Esc)
      if (e.key === 'Escape' && mainPanelMode !== 'terminal') {
        e.preventDefault();
        handleBackToTerminal();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [showAddServer, showSettings, showCloneRepo, showCreateFeature, showCreateSession, showCommandPalette, toggleDiffPanel, mainPanelMode, handleBackToTerminal]);

  // Breadcrumb segments derived from selection state
  const breadcrumbSegments = useMemo(() => {
    if (!selectedServer || (!selectedFeature && !selectedSession)) return null;

    const segments: { label: string; type: 'server' | 'worktree' | 'agent' }[] = [];

    // Server segment
    segments.push({ label: selectedServer.name, type: 'server' });

    // Worktree/feature segment
    const worktreeName = selectedSession?.worktree || selectedFeature?.name;
    if (worktreeName) {
      segments.push({ label: worktreeName, type: 'worktree' });
    }

    // Agent/session segment
    if (selectedSession?.windowName) {
      const parts = selectedSession.windowName.split(':');
      const agentName = parts.length > 1 ? parts[parts.length - 1] : selectedSession.windowName;
      segments.push({ label: agentName, type: 'agent' });
    }

    return segments;
  }, [selectedServer, selectedFeature, selectedSession]);

  // Determine what to show in the terminal
  const hasTerminal = selectedServer && (selectedFeature || selectedSession);

  // Compute a terminal key that changes only when the server or worktree context changes
  // (NOT when switching sessions within the same worktree)
  const terminalKey = selectedServer
    ? selectedSession
      ? `${selectedServer.id}:${selectedSession.repo}:${selectedSession.worktree}`
      : selectedFeature
        ? `${selectedServer.id}:feature:${selectedFeature.name}`
        : selectedServer.id
    : '';

  // Reset connection state and panel mode when terminal context changes (prevents stale indicator/view)
  useEffect(() => {
    setConnectionState(null);
    setMainPanelMode('terminal');
    setSelectedFilePath(null);
  }, [terminalKey]);

  return (
    <div className="flex flex-col h-screen bg-background text-foreground font-sans">
      {/* Header */}
      <header className="flex items-center justify-between px-4 py-2 border-b border-border bg-card shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-lg font-bold text-primary">NomadFlow</span>
        </div>
        <div className="flex items-center gap-3">
          {/* Breadcrumb navigation */}
          {breadcrumbSegments ? (
            <nav
              aria-label="Breadcrumb"
              role="navigation"
              className="flex items-center gap-1.5">
              {breadcrumbSegments.map((segment, i) => (
                <span key={segment.type} className="flex items-center gap-1.5">
                  {i > 0 && (
                    <ChevronRight size={12} className="text-muted-foreground" />
                  )}
                  <span
                    className={`text-[13px] max-w-40 overflow-hidden text-ellipsis whitespace-nowrap ${
                      segment.type === 'worktree'
                        ? 'text-foreground cursor-pointer'
                        : 'text-muted-foreground cursor-default'
                    }`}
                    title={segment.label}>
                    {segment.label}
                  </span>
                </span>
              ))}
            </nav>
          ) : (
            <span className="text-[13px] text-muted-foreground">
              {t('app.breadcrumb.select_worktree')}
            </span>
          )}

          {/* Connection status indicator */}
          {hasTerminal && connectionState && (
            <div className="flex items-center gap-1.5">
              <span
                className={`w-2 h-2 rounded-full inline-block ${
                  connectionState.status === 'connected'
                    ? 'bg-success'
                    : connectionState.status === 'reconnecting' ||
                        connectionState.status === 'connecting'
                      ? 'bg-warning'
                      : 'bg-destructive'
                } ${
                  connectionState.status === 'reconnecting'
                    ? 'animate-pulse-dot motion-reduce:animate-none'
                    : ''
                }`}
              />
              <span className="text-xs text-muted-foreground">
                {connectionState.status === 'connected'
                  ? t('terminal.status.connected')
                  : connectionState.status === 'reconnecting'
                    ? t('terminal.status.reconnecting', { attempts: connectionState.reconnectAttempts })
                    : connectionState.status === 'connecting'
                      ? t('terminal.status.connecting')
                      : connectionState.status === 'error'
                        ? t('common.error')
                        : t('terminal.status.disconnected')}
              </span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={toggleTheme}
            className="bg-transparent border-none cursor-pointer px-2.5 py-1.5 rounded-md text-muted-foreground text-sm"
            title={colorScheme === 'dark' ? t('app.theme_toggle_to_light') : t('app.theme_toggle_to_dark')}>
            {colorScheme === 'dark' ? <Moon size={14} /> : <Sun size={14} />}
          </button>
          <button
            onClick={() => setShowSettings(true)}
            className="bg-transparent border-none cursor-pointer px-2.5 py-1.5 rounded-md text-muted-foreground text-sm"
            title={t('app.settings')}
            aria-label={t('app.settings')}>
            <Settings size={14} />
          </button>
        </div>
      </header>

      {/* Main content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <Sidebar
          onSelectFeature={handleSelectFeature}
          onSelectSession={handleSelectSession}
          onCreateSession={handleCreateSession}
          onSessionsUpdate={handleSessionsUpdate}
          onAddServer={() => {
            setEditingServer(null);
            setShowAddServer(true);
          }}
          onEditServer={handleEditServer}
          onCloneRepo={handleCloneRepo}
          onCreateFeature={handleCreateFeature}
        />

        {/* Main panel */}
        <main className="flex-1 flex flex-col overflow-hidden">
          {hasTerminal ? (
            <>
              {/* Tab bar (only in terminal mode when session-based and has worktree sessions) */}
              {mainPanelMode === 'terminal' && selectedSession && worktreeSessions.length > 0 && (
                <AgentTabBar
                  sessions={worktreeSessions}
                  activeSessionId={selectedSession.sessionId}
                  onSelectSession={handleTabBarSelectSession}
                  onCreateSession={handleTabBarCreateSession}
                />
              )}
              {mainPanelMode === 'terminal' && selectedSession && worktreeSessions.length === 0 && (
                <div
                  className="mx-3 my-1.5 flex items-center gap-2 px-3 py-2 backdrop-blur-[12px] bg-[rgba(15,15,23,0.9)] rounded-[10px] border border-[rgba(255,255,255,0.06)] shrink-0">
                  <span className="text-[13px] text-muted-foreground">{t('agents.none_running')}</span>
                  <button
                    onClick={handleTabBarCreateSession}
                    className="px-2 py-1 text-[12px] rounded-md border-none bg-accent text-foreground cursor-pointer hover:bg-accent/80">
                    {t('agents.create.button_short')}
                  </button>
                </div>
              )}

              {/* Terminal — always mounted, hidden via CSS when not active (preserves WebSocket/xterm state) */}
              <div className="flex-1 flex-col" style={{ display: mainPanelMode === 'terminal' ? 'flex' : 'none' }}>
                <SplitTerminal
                  key={terminalKey}
                  ref={splitTerminalRef}
                  server={selectedServer!}
                  repoPath={selectedRepo?.path || '~'}
                  featureName={selectedFeature?.name || selectedSession?.worktree || ''}
                  sessionId={selectedSession?.sessionId}
                  worktreePath={currentWorktreePath}
                  shellSessions={shellSessions}
                  dialogOpen={showAddServer || showSettings || showCloneRepo || showCreateFeature || showCreateSession || showCommandPalette}
                  onConnectionStateChange={setConnectionState}
                  onToggleDiff={toggleDiffPanel}
                />
              </div>

              {/* Diff view — shown when file clicked in diff panel */}
              {mainPanelMode === 'diff' && selectedFilePath && (
                <FileDiffView
                  server={selectedServer!}
                  worktreePath={
                    selectedSession?.worktree ||
                    selectedFeature?.worktreePath ||
                    ''
                  }
                  filePath={selectedFilePath}
                  onBack={handleBackToTerminal}
                  onViewFile={() => setMainPanelMode('file-content')}
                />
              )}

              {/* File content view — shown when "View File" clicked from diff view */}
              {mainPanelMode === 'file-content' && selectedFilePath && (
                <FileContentView
                  server={selectedServer!}
                  worktreePath={
                    selectedSession?.worktree ||
                    selectedFeature?.worktreePath ||
                    ''
                  }
                  filePath={selectedFilePath}
                  onBack={handleBackToTerminal}
                  onViewDiff={() => setMainPanelMode('diff')}
                />
              )}
            </>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-3">
              <span className="opacity-30"><Sparkles size={48} /></span>
              <p className="text-base">{t('app.empty.select_worktree')}</p>
              <p className="text-[13px] opacity-70">
                {t('app.empty.navigate_sidebar')}
              </p>
            </div>
          )}
        </main>

        {/* Diff panel (3rd column) — always rendered when terminal active, animated via width */}
        {hasTerminal && selectedServer && (
          <aside
            aria-label={t('app.git_changes_panel')}
            className="flex flex-col overflow-hidden backdrop-blur-[20px] bg-[rgba(15,15,23,0.8)] border-l border-[rgba(255,255,255,0.06)] transition-[width,min-width] duration-200 ease-in-out motion-reduce:transition-none shrink-0"
            style={{
              width: showDiffPanel ? 280 : 0,
              minWidth: showDiffPanel ? 280 : 0,
            }}>
            {showDiffPanel && (
              <DiffPanel
                server={selectedServer}
                worktreePath={
                  selectedSession?.worktree ||
                  selectedFeature?.worktreePath ||
                  ''
                }
                onFileClick={handleFileClick}
              />
            )}
          </aside>
        )}
      </div>

      {/* Dialogs */}
      {showAddServer && (
        <AddServerDialog
          server={editingServer}
          onClose={() => {
            setShowAddServer(false);
            setEditingServer(null);
          }}
        />
      )}

      {showSettings && (
        <SettingsDialog
          colorScheme={colorScheme}
          onToggleTheme={toggleTheme}
          onClose={() => setShowSettings(false)}
        />
      )}

      {showCloneRepo && cloneRepoServer && (
        <CloneRepoDialog
          server={cloneRepoServer}
          onClose={() => {
            setShowCloneRepo(false);
            setCloneRepoServer(null);
          }}
        />
      )}

      {showCreateFeature && createFeatureServer && (
        <CreateFeatureDialog
          server={createFeatureServer}
          repoPath={createFeatureRepoPath}
          onClose={() => setShowCreateFeature(false)}
          onCreated={handleFeatureCreated}
        />
      )}

      {showCreateSession && createSessionServer && (
        <CreateSessionDialog
          server={createSessionServer}
          worktreePath={createSessionWorktreePath}
          onClose={() => setShowCreateSession(false)}
        />
      )}

      {showCommandPalette && (
        <CommandPalette
          allSessionsByServer={allSessionsByServer}
          onSelectSession={handleSelectSession}
          onSelectFeature={handleSelectFeature}
          onClose={() => setShowCommandPalette(false)}
          onAddServer={() => {
            setEditingServer(null);
            setShowAddServer(true);
          }}
          onOpenSettings={() => setShowSettings(true)}
          onSplitHorizontal={() => splitTerminalRef.current?.addPane('horizontal')}
          onSplitVertical={() => splitTerminalRef.current?.addPane('vertical')}
        />
      )}
    </div>
  );
}

export default function RootLayout() {
  return (
    <StorageProvider>
      <WebApp />
    </StorageProvider>
  );
}
