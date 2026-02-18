import { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { executeServerCommand } from '@/lib/server-commands';
import type { Server } from '@shared';

interface CreateSessionDialogProps {
  server: Server;
  worktreePath: string;
  onClose: () => void;
}

export function CreateSessionDialog({ server, worktreePath, onClose }: CreateSessionDialogProps) {
  const { t } = useTranslation();
  const [agentType, setAgentType] = useState('agent');
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState('');

  const worktreeName = worktreePath.split('/').pop() || worktreePath;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onClose]);

  const handleSubmit = useCallback(async () => {
    setIsCreating(true);
    setError('');

    try {
      const result = await executeServerCommand(server, {
        action: 'create-session',
        params: { worktreePath, agentType },
      });

      if (result.success) {
        onClose();
      } else {
        throw new Error(result.error || t('agents.create.error.failed'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('common.error.creation_failed'));
    } finally {
      setIsCreating(false);
    }
  }, [server, worktreePath, agentType, onClose, t]);

  return (
    <div
      onClick={onClose}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-[9999]">
      <div
        onClick={(e) => e.stopPropagation()}
        className="bg-card border border-border rounded-xl p-6 w-[380px] max-w-[90vw] shadow-2xl flex flex-col gap-4">
        <h2 className="text-lg font-semibold text-foreground m-0">{t('agents.create.title')}</h2>

        {/* Worktree path (read-only) */}
        <div>
          <label className="block text-[13px] font-medium text-foreground mb-1">{t('agents.create.label.worktree')}</label>
          <div className="w-full px-3 py-2 border border-border rounded-lg bg-muted text-muted-foreground text-sm">
            {worktreeName}
          </div>
        </div>

        {/* Agent type selector */}
        <div>
          <label className="block text-[13px] font-medium text-foreground mb-1">{t('agents.create.label.agent_type')}</label>
          <select
            value={agentType}
            onChange={(e) => setAgentType(e.target.value)}
            className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none cursor-pointer">
            <option value="agent">{t('agents.type.agent_default')}</option>
            <option value="claude-code">{t('agents.type.claude_code')}</option>
          </select>
        </div>

        {error && <p className="text-destructive text-[13px] m-0">{error}</p>}

        {/* Actions */}
        <div className="flex gap-3 justify-end mt-2">
          <button
            onClick={onClose}
            className="px-4 py-2 border border-border rounded-lg bg-transparent text-foreground text-sm cursor-pointer hover:bg-accent">
            {t('common.cancel')}
          </button>
          <button
            onClick={handleSubmit}
            disabled={isCreating}
            className="px-4 py-2 border-none rounded-lg bg-primary text-primary-foreground text-sm font-medium cursor-pointer disabled:opacity-50 disabled:cursor-wait">
            {isCreating ? t('common.creating') : t('common.create')}
          </button>
        </div>
      </div>
    </div>
  );
}
