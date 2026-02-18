import { useTranslation } from 'react-i18next';
import { Plus } from 'lucide-react-native';
import { AgentStatusBadge } from '@/components/shared/AgentStatusBadge';
import type { SessionWithState } from '@/lib/types/session';

interface AgentTabBarProps {
  sessions: SessionWithState[];
  activeSessionId: string | null;
  onSelectSession: (session: SessionWithState) => void;
  onCreateSession: () => void;
}

export function AgentTabBar({
  sessions,
  activeSessionId,
  onSelectSession,
  onCreateSession,
}: AgentTabBarProps) {
  const { t } = useTranslation();
  return (
    <div
      className="mx-3 my-1.5 flex items-center gap-1 px-2 py-1.5 overflow-x-auto backdrop-blur-[12px] bg-[rgba(15,15,23,0.9)] rounded-[10px] border border-[rgba(255,255,255,0.06)] shrink-0"
      role="tablist"
      aria-label={t('agents.sessions_label')}
      style={{ scrollbarWidth: 'none' }}>
      {sessions.map((session) => {
        const isActive = session.sessionId === activeSessionId;
        return (
          <button
            key={session.sessionId}
            onClick={() => onSelectSession(session)}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md border-none cursor-pointer text-[13px] whitespace-nowrap shrink-0 ${
              isActive
                ? 'bg-accent text-foreground'
                : 'bg-transparent text-muted-foreground hover:bg-accent/50'
            }`}
            role="tab"
            aria-selected={isActive}
            aria-current={isActive ? 'true' : undefined}>
            <AgentStatusBadge state={session.agentState} size="sm" />
            <span>
              {session.agentType}-{session.agentNumber}
            </span>
          </button>
        );
      })}

      <button
        onClick={onCreateSession}
        className="flex items-center justify-center w-7 h-7 rounded-md border-none bg-transparent text-muted-foreground hover:text-foreground hover:bg-accent/50 cursor-pointer text-sm shrink-0"
        title={t('agents.new_session')}
        aria-label={t('agents.create_session')}>
        <Plus size={14} />
      </button>
    </div>
  );
}
