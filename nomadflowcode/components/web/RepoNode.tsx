import { useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronRight, FolderOpen } from 'lucide-react-native';
import { executeServerCommand } from '@/lib/server-commands';
import { WorktreeNode } from './WorktreeNode';
import type { Server, Repository, Feature } from '@shared';
import type { SessionWithState } from '@/lib/types/session';

interface RepoNodeProps {
  server: Server;
  repo: Repository;
  sessions: SessionWithState[];
  onSelectFeature: (server: Server, repo: Repository, feature: Feature) => void;
  onSelectSession: (session: SessionWithState, worktreePath: string) => void;
  onCreateSession: (worktreePath: string) => void;
  onCreateFeature: (server: Server, repoPath: string) => void;
}

export function RepoNode({
  server,
  repo,
  sessions,
  onSelectFeature,
  onSelectSession,
  onCreateSession,
  onCreateFeature,
}: RepoNodeProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [features, setFeatures] = useState<Feature[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Group sessions by worktree (feature name) for efficient lookup
  const sessionsByFeature = useMemo(() => {
    const map = new Map<string, SessionWithState[]>();
    for (const session of sessions) {
      const existing = map.get(session.worktree);
      if (existing) {
        existing.push(session);
      } else {
        map.set(session.worktree, [session]);
      }
    }
    return map;
  }, [sessions]);

  const loadFeatures = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await executeServerCommand(server, {
        action: 'list-features',
        params: { repoPath: repo.path },
      });
      if (result.success && result.data) {
        setFeatures(result.data.features);
      } else {
        throw new Error(result.error || t('servers.error.load_failed'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('features.error.load_failed'));
    } finally {
      setIsLoading(false);
    }
  }, [server, repo.path]);

  const handleToggle = useCallback(() => {
    if (!expanded) {
      loadFeatures();
    }
    setExpanded((prev) => !prev);
  }, [expanded, loadFeatures]);

  const handleDeleteFeature = useCallback(
    async (feature: Feature) => {
      if (feature.isMain) return;
      if (
        !window.confirm(
          t('features.delete.confirm_web', { name: feature.name })
        )
      ) {
        return;
      }
      try {
        await executeServerCommand(server, {
          action: 'delete-feature',
          params: { repoPath: repo.path, featureName: feature.name },
        });
        loadFeatures();
      } catch {
        alert(t('features.delete.failed'));
      }
    },
    [server, repo.path, loadFeatures]
  );

  return (
    <div className="mb-0.5">
      <div
        onClick={handleToggle}
        className="flex items-center gap-2 px-2 py-[5px] cursor-pointer rounded-md select-none hover:bg-accent"
        role="treeitem"
        aria-expanded={expanded}>
        <ChevronRight
          size={10}
          className={`text-muted-foreground transition-transform duration-150 ${expanded ? 'rotate-90' : ''}`}
        />
        <FolderOpen size={14} />
        <span className="flex-1 text-[13px] overflow-hidden text-ellipsis whitespace-nowrap">
          {repo.name}
        </span>
        <span className="text-[11px] text-success">{repo.branch}</span>
      </div>

      {expanded && (
        <div className="pl-4">
          {isLoading && (
            <div className="px-2 py-1 text-xs text-muted-foreground">{t('common.loading')}</div>
          )}
          {error && <div className="px-2 py-1 text-xs text-destructive">{error}</div>}
          {!isLoading &&
            !error &&
            features.map((feature) => (
              <WorktreeNode
                key={feature.name}
                feature={feature}
                sessions={sessionsByFeature.get(feature.name) ?? []}
                onSelect={() => onSelectFeature(server, repo, feature)}
                onSelectSession={onSelectSession}
                onCreateSession={onCreateSession}
                onDelete={() => handleDeleteFeature(feature)}
              />
            ))}
          {!isLoading && !error && features.length === 0 && (
            <div className="px-2 py-1 text-xs text-muted-foreground">{t('features.empty.short')}</div>
          )}
          <button
            onClick={(e) => {
              e.stopPropagation();
              onCreateFeature(server, repo.path);
            }}
            className="flex items-center gap-1.5 px-2 py-[3px] my-0.5 border-none bg-transparent cursor-pointer text-xs text-muted-foreground rounded w-full text-left hover:text-primary">
            {t('features.create.button_short')}
          </button>
        </div>
      )}
    </div>
  );
}
