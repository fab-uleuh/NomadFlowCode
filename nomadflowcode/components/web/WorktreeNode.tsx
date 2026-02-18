import { useState, useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronRight, Home, GitBranch, Plus } from 'lucide-react-native';
import type { Feature } from '@shared';
import type { AgentStateKind, SessionWithState } from '@/lib/types/session';
import { AgentStatusBadge } from '@/components/shared/AgentStatusBadge';

const STATE_PRIORITY: Record<AgentStateKind, number> = {
  error: 0,
  waiting_for_input: 1,
  generating: 2,
  idle: 3,
  done: 4,
  unknown: 5,
};

const STATE_LABEL_KEY: Record<AgentStateKind, string> = {
  error: 'agents.state.error',
  waiting_for_input: 'agents.state.waiting',
  generating: 'agents.state.generating',
  idle: 'agents.state.idle',
  done: 'agents.state.done',
  unknown: '',
};

const STATE_COLOR: Record<AgentStateKind, string> = {
  error: 'text-destructive',
  waiting_for_input: 'text-warning',
  generating: 'text-primary',
  idle: 'text-muted-foreground',
  done: 'text-success',
  unknown: 'text-muted',
};

function aggregateState(
  sessions: SessionWithState[]
): { state: AgentStateKind; count: number } | null {
  if (sessions.length === 0) return null;
  const worst = sessions.reduce((a, b) =>
    STATE_PRIORITY[a.agentState] <= STATE_PRIORITY[b.agentState] ? a : b
  );
  const count = sessions.filter((s) => s.agentState === worst.agentState).length;
  return { state: worst.agentState, count };
}

interface WorktreeNodeProps {
  feature: Feature;
  sessions: SessionWithState[];
  onSelect: () => void;
  onSelectSession: (session: SessionWithState, worktreePath: string) => void;
  onCreateSession: (worktreePath: string) => void;
  onDelete: () => void;
}

export function WorktreeNode({
  feature,
  sessions,
  onSelect,
  onSelectSession,
  onCreateSession,
  onDelete,
}: WorktreeNodeProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [showContextMenu, setShowContextMenu] = useState(false);

  const WorktreeIcon = feature.isMain ? Home : GitBranch;

  const aggregated = useMemo(() => aggregateState(sessions), [sessions]);

  const handleToggleExpand = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setExpanded((prev) => !prev);
  }, []);

  const handleClick = useCallback(() => {
    if (sessions.length > 0) {
      onSelectSession(sessions[0], feature.worktreePath);
    } else {
      onSelect();
    }
    // Auto-expand on navigate (but never collapse)
    if (!expanded) setExpanded(true);
  }, [expanded, sessions, onSelectSession, onSelect, feature.worktreePath]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      if (!feature.isMain) {
        setShowContextMenu(true);
      }
    },
    [feature.isMain]
  );

  useEffect(() => {
    if (showContextMenu) {
      const handler = () => setShowContextMenu(false);
      document.addEventListener('click', handler);
      return () => document.removeEventListener('click', handler);
    }
  }, [showContextMenu]);

  const nameColor = feature.isMain
    ? 'text-warning'
    : feature.isActive
      ? 'text-success'
      : 'text-foreground';

  return (
    <div className="relative mb-0.5">
      {/* Worktree header row */}
      <div
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        className="flex items-center gap-2 px-2 py-1 cursor-pointer rounded-md select-none text-[13px] hover:bg-accent"
        role="treeitem"
        aria-expanded={expanded}>
        <span
          onClick={handleToggleExpand}
          className={`inline-flex transition-transform duration-150 cursor-pointer ${expanded ? 'rotate-90' : ''}`}>
          <ChevronRight size={10} className="text-muted-foreground" />
        </span>
        <WorktreeIcon
          size={14}
          className={!feature.isMain && !feature.isActive ? 'text-muted-foreground' : undefined}
        />
        <span
          className={`flex-1 overflow-hidden text-ellipsis whitespace-nowrap ${nameColor} ${feature.isActive ? 'font-medium' : 'font-normal'}`}>
          {feature.name}
        </span>

        {/* Aggregated state badge (collapsed only) */}
        {!expanded && aggregated && (
          <div className="flex items-center gap-1 text-[11px] shrink-0">
            <AgentStatusBadge state={aggregated.state} size="sm" />
            <span className={STATE_COLOR[aggregated.state]}>
              {aggregated.count} {STATE_LABEL_KEY[aggregated.state] ? t(STATE_LABEL_KEY[aggregated.state]) : '\u2014'}
            </span>
          </div>
        )}

        {/* Create session button */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            onCreateSession(feature.worktreePath);
          }}
          className="text-muted-foreground hover:text-primary text-xs border-none bg-transparent cursor-pointer px-1 shrink-0"
          title={t('agents.new_session')}
          aria-label={t('agents.new_session')}>
          <Plus size={12} />
        </button>

        {feature.isMain && (
          <span className="text-[10px] px-1.5 py-px rounded-[10px] bg-warning text-white font-semibold shrink-0">
            {t('features.badge.source')}
          </span>
        )}
      </div>

      {/* Session rows (expanded) */}
      {expanded && sessions.length > 0 && (
        <div className="pl-8" role="group">
          {sessions.map((session) => (
            <div
              key={session.sessionId}
              onClick={(e) => {
                e.stopPropagation();
                onSelectSession(session, feature.worktreePath);
              }}
              className="flex items-center gap-2 px-2 py-1 cursor-pointer rounded-md select-none text-xs hover:bg-accent">
              <span className="text-muted-foreground">
                {session.agentType}-{session.agentNumber}
              </span>
              <AgentStatusBadge
                state={session.agentState}
                size="md"
                agentName={`${session.agentType}-${session.agentNumber}`}
              />
            </div>
          ))}
        </div>
      )}

      {/* Context menu */}
      {showContextMenu && (
        <div className="absolute top-full left-2 z-[1000] bg-popover border border-border rounded-lg p-1 min-w-[120px] shadow-lg">
          <button
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
              setShowContextMenu(false);
            }}
            className="block w-full px-3 py-1.5 border-none bg-transparent cursor-pointer text-left text-[13px] rounded text-destructive hover:bg-accent">
            {t('common.delete')}
          </button>
        </div>
      )}
    </div>
  );
}
