import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { executeServerCommand } from '@/lib/server-commands';
import type { Server } from '@shared';

interface CloneRepoDialogProps {
  server: Server;
  onClose: () => void;
}

export function CloneRepoDialog({ server, onClose }: CloneRepoDialogProps) {
  const { t } = useTranslation();
  const [url, setUrl] = useState('');
  const [token, setToken] = useState('');
  const [name, setName] = useState('');
  const [isCloning, setIsCloning] = useState(false);
  const [error, setError] = useState('');

  const handleClone = async () => {
    const trimmedUrl = url.trim();
    if (!trimmedUrl) {
      setError(t('repos.clone.error.url_required'));
      return;
    }

    setIsCloning(true);
    setError('');

    try {
      const params: Record<string, string> = { url: trimmedUrl };
      if (token.trim()) params.token = token.trim();
      if (name.trim()) params.name = name.trim();

      const result = await executeServerCommand(server, {
        action: 'clone-repo',
        params,
      });

      if (result.success) {
        onClose();
      } else {
        throw new Error(result.error || t('repos.clone.error.failed'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('repos.clone.error.failed'));
    } finally {
      setIsCloning(false);
    }
  };

  return (
    <div
      onClick={onClose}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-[9999]">
      <div
        onClick={(e) => e.stopPropagation()}
        className="bg-card rounded-xl border border-border p-6 w-[420px] max-w-[90vw] shadow-[0_8px_32px_rgba(0,0,0,0.25)]">
        <h2 className="text-lg font-semibold mb-5">{t('repos.clone.title')}</h2>

        <div className="flex flex-col gap-4">
          <div>
            <label className="block text-[13px] font-medium mb-1 text-foreground">
              {t('repos.clone.label.url')}
            </label>
            <input
              type="url"
              placeholder={t('repos.clone.placeholder.url')}
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
              autoFocus
            />
          </div>

          <div>
            <label className="block text-[13px] font-medium mb-1 text-foreground">
              {t('repos.clone.label.token')}
            </label>
            <input
              type="password"
              placeholder={t('repos.clone.placeholder.token')}
              value={token}
              onChange={(e) => setToken(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
            />
          </div>

          <div>
            <label className="block text-[13px] font-medium mb-1 text-foreground">
              {t('repos.clone.label.name')}
            </label>
            <input
              type="text"
              placeholder={t('repos.clone.placeholder.name')}
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
            />
          </div>

          {error && (
            <p className="text-destructive text-[13px] m-0">{error}</p>
          )}

          <div className="flex gap-3 justify-end mt-1">
            <button
              onClick={onClose}
              className="px-4 py-2 border border-border rounded-lg bg-transparent text-foreground text-sm cursor-pointer">
              {t('common.cancel')}
            </button>
            <button
              onClick={handleClone}
              disabled={isCloning}
              className={`px-4 py-2 border-none rounded-lg bg-primary text-primary-foreground text-sm font-medium cursor-pointer ${
                isCloning ? 'opacity-70 cursor-wait' : ''
              }`}>
              {isCloning ? t('repos.clone.cloning') : t('repos.clone.button')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
