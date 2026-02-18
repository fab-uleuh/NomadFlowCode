import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useStorage } from '@/lib/context/storage-context';
import type { Server } from '@shared';

interface AddServerDialogProps {
  server: Server | null;
  onClose: () => void;
}

export function AddServerDialog({ server, onClose }: AddServerDialogProps) {
  const { t } = useTranslation();
  const { addServer, updateServer } = useStorage();
  const isEditing = !!server;

  const [name, setName] = useState('');
  const [apiUrl, setApiUrl] = useState('');
  const [authToken, setAuthToken] = useState('');
  const [error, setError] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (server) {
      setName(server.name);
      setApiUrl(server.apiUrl || '');
      setAuthToken(server.authToken || '');
    }
  }, [server]);

  const handleSubmit = async () => {
    if (!name.trim()) {
      setError(t('servers.add.error.name_required'));
      return;
    }
    if (!apiUrl.trim()) {
      setError(t('servers.add.error.url_required'));
      return;
    }
    if (!apiUrl.startsWith('http://') && !apiUrl.startsWith('https://')) {
      setError(t('servers.add.error.url_invalid_protocol'));
      return;
    }

    setIsSubmitting(true);
    setError('');

    try {
      const serverData = {
        name: name.trim(),
        apiUrl: apiUrl.trim(),
        authToken: authToken.trim() || undefined,
      };

      if (isEditing && server) {
        await updateServer(server.id, serverData);
      } else {
        await addServer(serverData);
      }
      onClose();
    } catch {
      setError(t('servers.add.error.save_failed'));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div
      onClick={onClose}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-[9999]">
      <div
        onClick={(e) => e.stopPropagation()}
        className="bg-card rounded-xl border border-border p-6 w-[420px] max-w-[90vw] shadow-[0_8px_32px_rgba(0,0,0,0.25)]">
        <h2 className="text-lg font-semibold mb-5">
          {isEditing ? t('servers.edit.title') : t('servers.add.title')}
        </h2>

        <div className="flex flex-col gap-4">
          <div>
            <label className="block text-[13px] font-medium mb-1 text-foreground">
              {t('servers.add.label.name')}
            </label>
            <input
              type="text"
              placeholder={t('servers.add.placeholder.name')}
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
              autoFocus
            />
          </div>

          <div>
            <label className="block text-[13px] font-medium mb-1 text-foreground">
              {t('servers.add.label.url')}
            </label>
            <input
              type="url"
              placeholder="http://192.168.1.100:8080"
              value={apiUrl}
              onChange={(e) => setApiUrl(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t('servers.add.hint.url')}
            </p>
          </div>

          <div>
            <label className="block text-[13px] font-medium mb-1 text-foreground">
              {t('servers.add.label.secret')}
            </label>
            <input
              type="password"
              placeholder={t('servers.add.placeholder.secret')}
              value={authToken}
              onChange={(e) => setAuthToken(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t('servers.add.hint.secret')}
            </p>
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
              onClick={handleSubmit}
              disabled={isSubmitting}
              className={`px-4 py-2 border-none rounded-lg bg-primary text-primary-foreground text-sm font-medium cursor-pointer ${
                isSubmitting ? 'opacity-70 cursor-wait' : ''
              }`}>
              {isSubmitting ? t('common.saving') : isEditing ? t('common.update') : t('servers.add.submit')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
