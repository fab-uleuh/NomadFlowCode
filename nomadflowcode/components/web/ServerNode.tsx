import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronRight, Monitor, MoreHorizontal } from 'lucide-react-native';
import { executeServerCommand } from '@/lib/server-commands';
import { useStorage } from '@/lib/context/storage-context';
import { useAgentStatePolling } from '@/lib/hooks/useAgentStatePolling';
import { RepoNode } from './RepoNode';
import type { Server, Repository, Feature } from '@shared';
import type { SessionWithState } from '@/lib/types/session';

interface ServerNodeProps {
  server: Server;
  onSelectFeature: (server: Server, repo: Repository, feature: Feature) => void;
  onSelectSession: (server: Server, session: SessionWithState, worktreePath: string) => void;
  onCreateSession: (server: Server, worktreePath: string) => void;
  onSessionsUpdate: (server: Server, sessions: SessionWithState[]) => void;
  onEditServer: (server: Server) => void;
  onCloneRepo: (server: Server) => void;
  onCreateFeature: (server: Server, repoPath: string) => void;
}

export function ServerNode({
  server,
  onSelectFeature,
  onSelectSession,
  onCreateSession,
  onSessionsUpdate,
  onEditServer,
  onCloneRepo,
  onCreateFeature,
}: ServerNodeProps) {
  const { t } = useTranslation();
  const { deleteServer } = useStorage();
  const [expanded, setExpanded] = useState(false);
  const [repos, setRepos] = useState<Repository[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [online, setOnline] = useState<boolean | null>(null);
  const [showMenu, setShowMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  const { sessions } = useAgentStatePolling(server.apiUrl ?? '', server.authToken ?? '');

  // Bubble up polling results to parent for tab bar
  useEffect(() => {
    onSessionsUpdate(server, sessions);
  }, [sessions, server, onSessionsUpdate]);

  // Group sessions by repo name for efficient lookup in child components
  const sessionsByRepo = useMemo(() => {
    const map = new Map<string, SessionWithState[]>();
    for (const session of sessions) {
      const existing = map.get(session.repo);
      if (existing) {
        existing.push(session);
      } else {
        map.set(session.repo, [session]);
      }
    }
    return map;
  }, [sessions]);

  const loadRepos = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await executeServerCommand(server, { action: 'list-repos' });
      if (result.success && result.data) {
        setRepos(result.data.repos);
        setOnline(true);
      } else {
        throw new Error(result.error || t('servers.error.load_failed'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('servers.error.connection_failed'));
      setOnline(false);
    } finally {
      setIsLoading(false);
    }
  }, [server]);

  const handleToggle = useCallback(() => {
    if (!expanded) {
      loadRepos();
    }
    setExpanded((prev) => !prev);
  }, [expanded, loadRepos]);

  // Close menu on click outside
  useEffect(() => {
    if (!showMenu) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setShowMenu(false);
      }
    };
    document.addEventListener('click', handler);
    return () => document.removeEventListener('click', handler);
  }, [showMenu]);

  const handleDelete = useCallback(() => {
    if (window.confirm(t('servers.delete.confirm_web', { name: server.name }))) {
      deleteServer(server.id);
    }
    setShowMenu(false);
  }, [server, deleteServer]);

  return (
    <div className="mb-0.5">
      <div
        onClick={handleToggle}
        className="group flex items-center gap-2 px-2 py-1.5 cursor-pointer rounded-md select-none relative hover:bg-accent"
        role="treeitem"
        aria-expanded={expanded}>
        <ChevronRight
          size={10}
          className={`text-muted-foreground transition-transform duration-150 ${expanded ? 'rotate-90' : ''}`}
        />
        <Monitor size={14} />
        <span className="flex-1 text-[13px] font-semibold overflow-hidden text-ellipsis whitespace-nowrap">
          {server.name}
        </span>
        <span
          className={`w-2 h-2 rounded-full shrink-0 ${
            online === true
              ? 'bg-success'
              : online === false
                ? 'bg-destructive'
                : 'bg-muted-foreground'
          }`}
        />

        {/* Three-dot menu button (visible on hover) */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            setShowMenu((prev) => !prev);
          }}
          className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground border-none bg-transparent cursor-pointer text-sm px-1 shrink-0"
          title={t('servers.options')}>
          <MoreHorizontal size={14} />
        </button>

        {/* Popover menu */}
        {showMenu && (
          <div
            ref={menuRef}
            className="absolute top-full right-2 z-[1000] bg-popover border border-border rounded-lg p-1 min-w-[140px] shadow-lg">
            <button
              onClick={(e) => {
                e.stopPropagation();
                onEditServer(server);
                setShowMenu(false);
              }}
              className="block w-full px-3 py-1.5 border-none bg-transparent cursor-pointer text-left text-[13px] rounded text-foreground hover:bg-accent">
              {t('common.edit')}
            </button>
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleDelete();
              }}
              className="block w-full px-3 py-1.5 border-none bg-transparent cursor-pointer text-left text-[13px] rounded text-destructive hover:bg-accent">
              {t('common.delete')}
            </button>
          </div>
        )}
      </div>

      {expanded && (
        <div className="pl-4">
          {isLoading && (
            <div className="px-2 py-1.5 text-xs text-muted-foreground">{t('common.loading')}</div>
          )}
          {error && <div className="px-2 py-1.5 text-xs text-destructive">{error}</div>}
          {!isLoading &&
            !error &&
            repos.map((repo) => (
              <RepoNode
                key={repo.path}
                server={server}
                repo={repo}
                sessions={sessionsByRepo.get(repo.name) ?? []}
                onSelectFeature={onSelectFeature}
                onSelectSession={(session, worktreePath) => onSelectSession(server, session, worktreePath)}
                onCreateSession={(worktreePath) => onCreateSession(server, worktreePath)}
                onCreateFeature={onCreateFeature}
              />
            ))}
          {!isLoading && !error && repos.length === 0 && (
            <div className="px-2 py-1.5 text-xs text-muted-foreground">{t('repos.empty.short')}</div>
          )}
          <button
            onClick={(e) => {
              e.stopPropagation();
              onCloneRepo(server);
            }}
            className="flex items-center gap-1.5 px-2 py-1 my-0.5 border-none bg-transparent cursor-pointer text-xs text-muted-foreground rounded w-full text-left hover:text-primary">
            {t('repos.clone.button_short')}
          </button>
        </div>
      )}
    </div>
  );
}
