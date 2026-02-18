import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Search, GitBranch, Command } from 'lucide-react-native';
import { useStorage } from '@/lib/context/storage-context';
import { executeServerCommand } from '@/lib/server-commands';
import { AgentStatusBadge } from '@/components/shared/AgentStatusBadge';
import type { Server, Repository, Feature } from '@shared';
import type { SessionWithState, AgentStateKind } from '@/lib/types/session';

type PaletteItemKind = 'session' | 'worktree' | 'action';

interface PaletteItem {
  id: string;
  kind: PaletteItemKind;
  label: string;
  sublabel?: string;
  agentState?: AgentStateKind;
  server?: Server;
  session?: SessionWithState;
  feature?: Feature;
  repo?: Repository;
  action?: () => void;
}

interface CommandPaletteProps {
  allSessionsByServer: Map<string, SessionWithState[]>;
  onSelectSession: (server: Server, session: SessionWithState, worktreePath: string) => void;
  onSelectFeature: (server: Server, repo: Repository, feature: Feature) => void;
  onClose: () => void;
  onAddServer: () => void;
  onOpenSettings: () => void;
  onSplitHorizontal?: () => void;
  onSplitVertical?: () => void;
}

export function CommandPalette({
  allSessionsByServer,
  onSelectSession,
  onSelectFeature,
  onClose,
  onAddServer,
  onOpenSettings,
  onSplitHorizontal,
  onSplitVertical,
}: CommandPaletteProps) {
  const { t } = useTranslation();
  const { servers } = useStorage();
  const [query, setQuery] = useState('');
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const [worktreeItems, setWorktreeItems] = useState<PaletteItem[]>([]);
  const [loadingServers, setLoadingServers] = useState<Set<string>>(new Set());
  const [fetchErrors, setFetchErrors] = useState<string[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Auto-focus search input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Build session items from allSessionsByServer (sync, instant — Tier 1)
  const sessionItems = useMemo(() => {
    const items: PaletteItem[] = [];
    for (const server of servers) {
      const sessions = allSessionsByServer.get(server.id) ?? [];
      for (const session of sessions) {
        items.push({
          id: `session:${server.id}:${session.sessionId}`,
          kind: 'session',
          label: `${session.agentType}-${session.agentNumber}`,
          sublabel: `${server.name} \u203A ${session.repo.split('/').pop()} \u203A ${session.worktree.split('/').pop()}`,
          agentState: session.agentState,
          server,
          session,
        });
      }
    }
    return items;
  }, [servers, allSessionsByServer]);

  // Build action items (static — Tier 3)
  const actionItems = useMemo(() => {
    const items: PaletteItem[] = [];

    items.push({
      id: 'action:add-server',
      kind: 'action',
      label: t('servers.add.button'),
      sublabel: t('servers.add.hint_palette'),
      action: onAddServer,
    });
    items.push({
      id: 'action:settings',
      kind: 'action',
      label: t('settings.title'),
      sublabel: t('settings.hint_palette'),
      action: onOpenSettings,
    });
    if (onSplitHorizontal) {
      items.push({
        id: 'action:split-h',
        kind: 'action',
        label: t('terminal.split.horizontal'),
        sublabel: '\u2318D',
        action: onSplitHorizontal,
      });
    }
    if (onSplitVertical) {
      items.push({
        id: 'action:split-v',
        kind: 'action',
        label: t('terminal.split.vertical'),
        sublabel: '\u2318\u21E7D',
        action: onSplitVertical,
      });
    }
    return items;
  }, [onAddServer, onOpenSettings, onSplitHorizontal, onSplitVertical]);

  // Fetch worktree items on mount (async — Tier 2, parallel per server)
  useEffect(() => {
    let cancelled = false;

    async function fetchWorktrees() {
      await Promise.all(servers.map(async (server) => {
        if (cancelled) return;
        setLoadingServers((prev) => new Set(prev).add(server.id));

        try {
          const reposResult = await executeServerCommand(server, {
            action: 'list-repos',
          });

          if (cancelled) return;

          const serverItems: PaletteItem[] = [];

          if (reposResult.success && reposResult.data?.repos) {
            const repos: Repository[] = reposResult.data.repos;

            await Promise.all(repos.map(async (repo) => {
              const featResult = await executeServerCommand(server, {
                action: 'list-features',
                params: { repoPath: repo.path },
              });

              if (cancelled) return;

              if (featResult.success && featResult.data?.features) {
                const features: Feature[] = featResult.data.features;
                for (const feature of features) {
                  serverItems.push({
                    id: `worktree:${server.id}:${repo.path}:${feature.name}`,
                    kind: 'worktree',
                    label: feature.name,
                    sublabel: `${server.name} \u203A ${repo.name}`,
                    server,
                    repo,
                    feature,
                  });
                }
              }
            }));
          }

          if (!cancelled) {
            setWorktreeItems((prev) => [...prev, ...serverItems]);
          }
        } catch {
          if (!cancelled) {
            setFetchErrors((prev) => [...prev, server.name]);
          }
        }

        if (!cancelled) {
          setLoadingServers((prev) => {
            const next = new Set(prev);
            next.delete(server.id);
            return next;
          });
        }
      }));
    }

    fetchWorktrees();
    return () => {
      cancelled = true;
    };
  }, [servers]);

  // Combine all items
  const allItems = useMemo(
    () => [...sessionItems, ...worktreeItems, ...actionItems],
    [sessionItems, worktreeItems, actionItems]
  );

  // Filter by query (case-insensitive includes on label and sublabel)
  const filteredItems = useMemo(() => {
    if (!query.trim()) return allItems;
    const q = query.toLowerCase();
    return allItems.filter(
      (item) =>
        item.label.toLowerCase().includes(q) ||
        (item.sublabel && item.sublabel.toLowerCase().includes(q))
    );
  }, [allItems, query]);

  // Group filtered items by kind with section headers
  const groupedItems = useMemo(() => {
    const sessions = filteredItems.filter((i) => i.kind === 'session');
    const worktrees = filteredItems.filter((i) => i.kind === 'worktree');
    const actions = filteredItems.filter((i) => i.kind === 'action');

    const result: { label: string; items: PaletteItem[] }[] = [];
    if (sessions.length > 0) result.push({ label: t('command_palette.section.sessions'), items: sessions });
    if (worktrees.length > 0) result.push({ label: t('command_palette.section.worktrees'), items: worktrees });
    if (actions.length > 0) result.push({ label: t('command_palette.section.actions'), items: actions });
    return result;
  }, [filteredItems]);

  // Pre-compute flat list with indices for keyboard navigation
  const renderData = useMemo(() => {
    let idx = 0;
    return groupedItems.map((group) => ({
      label: group.label,
      items: group.items.map((item) => ({ item, flatIndex: idx++ })),
    }));
  }, [groupedItems]);

  const flatItems = useMemo(
    () => groupedItems.flatMap((g) => g.items),
    [groupedItems]
  );

  // Reset highlighted index when query changes
  useEffect(() => {
    setHighlightedIndex(0);
  }, [query]);

  // Execute item action
  const executeItem = useCallback(
    (item: PaletteItem) => {
      if (item.kind === 'session' && item.server && item.session) {
        onSelectSession(item.server, item.session, item.session.worktree);
      } else if (item.kind === 'worktree' && item.server && item.repo && item.feature) {
        onSelectFeature(item.server, item.repo, item.feature);
      } else if (item.kind === 'action' && item.action) {
        item.action();
      }
      onClose();
    },
    [onSelectSession, onSelectFeature, onClose]
  );

  // Keyboard navigation on search input
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setHighlightedIndex((i) => Math.min(i + 1, flatItems.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setHighlightedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && flatItems[highlightedIndex]) {
        e.preventDefault();
        executeItem(flatItems[highlightedIndex]);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    },
    [flatItems, highlightedIndex, executeItem, onClose]
  );

  // Scroll highlighted item into view
  useEffect(() => {
    const el = listRef.current?.querySelector(`[data-index="${highlightedIndex}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [highlightedIndex]);

  const isLoading = loadingServers.size > 0;

  return (
    <div
      onClick={onClose}
      className="fixed inset-0 bg-black/40 z-[9999] flex justify-center pt-[15vh]">
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-[600px] max-w-[90vw] max-h-[60vh] bg-[rgba(15,15,23,0.95)] backdrop-blur-[20px] border border-[rgba(255,255,255,0.06)] rounded-xl shadow-[0_16px_48px_rgba(0,0,0,0.4)] overflow-hidden flex flex-col self-start">
        {/* Search input */}
        <div className="px-4 py-3 border-b border-[rgba(255,255,255,0.06)]">
          <div className="flex items-center gap-2.5">
            <Search size={16} className="text-muted-foreground" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t('command_palette.search_placeholder')}
              aria-label={t('command_palette.search_placeholder')}
              className="flex-1 bg-transparent border-none outline-none text-foreground text-base font-[inherit]"
            />
          </div>
        </div>

        {/* Results */}
        <div ref={listRef} className="overflow-y-auto py-2" role="listbox">
          {renderData.map((group) => (
            <div key={group.label}>
              <div className="px-4 py-1.5 text-[11px] font-semibold uppercase tracking-[0.05em] text-muted-foreground">
                {group.label}
              </div>
              {group.items.map(({ item, flatIndex }) => {
                const isHighlighted = flatIndex === highlightedIndex;
                return (
                  <div
                    key={item.id}
                    data-index={flatIndex}
                    role="option"
                    aria-selected={isHighlighted}
                    onClick={() => executeItem(item)}
                    onMouseEnter={() => setHighlightedIndex(flatIndex)}
                    className={`flex items-center gap-2.5 px-4 py-2 min-h-[44px] cursor-pointer border-l-2 ${
                      isHighlighted
                        ? 'bg-accent border-l-primary'
                        : 'bg-transparent border-l-transparent'
                    }`}>
                    {/* Icon */}
                    <span className="w-5 flex items-center justify-center shrink-0">
                      {item.kind === 'session' && item.agentState && (
                        <AgentStatusBadge state={item.agentState} size="sm" />
                      )}
                      {item.kind === 'worktree' && (
                        <GitBranch size={14} className="text-muted-foreground" />
                      )}
                      {item.kind === 'action' && (
                        <Command size={14} className="text-muted-foreground" />
                      )}
                    </span>
                    {/* Text */}
                    <div className="flex-1 min-w-0">
                      <div className="text-sm text-foreground whitespace-nowrap overflow-hidden text-ellipsis">
                        {item.label}
                      </div>
                      {item.sublabel && (
                        <div className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis">
                          {item.sublabel}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          ))}

          {/* Loading state */}
          {isLoading && (
            <div className="px-4 py-2 text-xs text-muted-foreground">
              {t('command_palette.loading')}
            </div>
          )}

          {/* Fetch error state */}
          {fetchErrors.length > 0 && (
            <div className="px-4 py-2 text-xs text-destructive">
              {t('command_palette.error.failed_load', { servers: fetchErrors.join(', ') })}
            </div>
          )}

          {/* Empty state */}
          {!isLoading && flatItems.length === 0 && query.trim() && (
            <div className="p-4 text-center text-muted-foreground text-sm">
              {t('command_palette.no_results', { query })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
