import { useTranslation } from 'react-i18next';
import { useStorage } from '@/lib/context/storage-context';
import { ServerNode } from './ServerNode';
import type { Server, Repository, Feature } from '@shared';
import type { SessionWithState } from '@/lib/types/session';

interface SidebarProps {
  onSelectFeature: (server: Server, repo: Repository, feature: Feature) => void;
  onSelectSession: (server: Server, session: SessionWithState, worktreePath: string) => void;
  onCreateSession: (server: Server, worktreePath: string) => void;
  onSessionsUpdate: (server: Server, sessions: SessionWithState[]) => void;
  onAddServer: () => void;
  onEditServer: (server: Server) => void;
  onCloneRepo: (server: Server) => void;
  onCreateFeature: (server: Server, repoPath: string) => void;
}

export function Sidebar({
  onSelectFeature,
  onSelectSession,
  onCreateSession,
  onSessionsUpdate,
  onAddServer,
  onEditServer,
  onCloneRepo,
  onCreateFeature,
}: SidebarProps) {
  const { t } = useTranslation();
  const { servers, isLoading } = useStorage();

  return (
    <aside className="w-[240px] min-w-[240px] flex flex-col overflow-hidden backdrop-blur-[20px] bg-[rgba(15,15,23,0.8)] border-r border-[rgba(255,255,255,0.06)]">
      {/* Header */}
      <div className="px-3 pt-2.5 pb-1.5 text-[11px] font-semibold uppercase tracking-[0.05em] text-muted-foreground">
        {t('servers.title')}
      </div>

      {/* Server list */}
      <div className="flex-1 overflow-y-auto px-1" role="tree">
        {isLoading ? (
          <div className="px-2 py-3 text-[13px] text-muted-foreground">{t('common.loading')}</div>
        ) : servers.length === 0 ? (
          <div className="px-3 py-6 text-center text-muted-foreground text-[13px]">
            {t('servers.empty.web_line1')}
            <br />
            {t('servers.empty.web_line2')}
          </div>
        ) : (
          servers.map((server) => (
            <ServerNode
              key={server.id}
              server={server}
              onSelectFeature={onSelectFeature}
              onSelectSession={onSelectSession}
              onCreateSession={onCreateSession}
              onSessionsUpdate={onSessionsUpdate}
              onEditServer={onEditServer}
              onCloneRepo={onCloneRepo}
              onCreateFeature={onCreateFeature}
            />
          ))
        )}
      </div>

      {/* Footer */}
      <div className="p-2 border-t border-border">
        <button
          onClick={onAddServer}
          className="w-full px-3 py-2 border border-border rounded-lg bg-transparent text-foreground text-[13px] cursor-pointer flex items-center justify-center gap-1.5 hover:bg-accent">
          {t('servers.add.button_short')}
        </button>
      </div>
    </aside>
  );
}
