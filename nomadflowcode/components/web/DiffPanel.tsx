import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { fetchWorktreeStatus } from '@/lib/server-commands';
import { RefreshCw, CheckCircle } from 'lucide-react-native';
import type { Server, FileChange, StatusSummary } from '@shared';

interface DiffPanelProps {
  server: Server;
  worktreePath: string;
  onFileClick?: (filePath: string) => void;
}

const STATUS_CONFIG: Record<string, { label: string; color: string }> = {
  modified: { label: 'M', color: 'text-amber-400' },
  new: { label: 'A', color: 'text-green-400' },
  deleted: { label: 'D', color: 'text-red-400' },
  renamed: { label: 'R', color: 'text-blue-400' },
  conflicted: { label: 'C', color: 'text-red-500' },
};

function FileRow({
  file,
  onClick,
}: {
  file: FileChange;
  onClick?: (filePath: string) => void;
}) {
  const config = STATUS_CONFIG[file.status] || { label: '?', color: 'text-muted-foreground' };
  const fileName = file.path.split('/').pop() || file.path;
  const dirPath = file.path.includes('/') ? file.path.slice(0, file.path.lastIndexOf('/')) : '';

  return (
    <button
      onClick={() => onClick?.(file.path)}
      className="w-full flex items-center gap-2 px-3 py-1.5 text-left hover:bg-[rgba(255,255,255,0.04)] rounded-md group"
      title={file.path}>
      <span className={`text-[11px] font-mono font-semibold w-4 text-center shrink-0 ${config.color}`}>
        {config.label}
      </span>
      <span className="flex-1 min-w-0 text-[13px] text-foreground truncate">
        {fileName}
        {dirPath && (
          <span className="text-muted-foreground text-[11px] ml-1">{dirPath}</span>
        )}
      </span>
      <span className="flex items-center gap-1 shrink-0 text-[11px] font-mono">
        {file.additions > 0 && (
          <span className="text-green-400">+{file.additions}</span>
        )}
        {file.deletions > 0 && (
          <span className="text-red-400">-{file.deletions}</span>
        )}
      </span>
    </button>
  );
}

function SkeletonRows() {
  return (
    <div className="px-3 py-2 flex flex-col gap-2">
      {[1, 2, 3].map((i) => (
        <div key={i} className="flex items-center gap-2 animate-pulse motion-reduce:animate-none">
          <div className="w-4 h-4 rounded bg-[rgba(255,255,255,0.06)]" />
          <div className="flex-1 h-4 rounded bg-[rgba(255,255,255,0.06)]" />
          <div className="w-10 h-4 rounded bg-[rgba(255,255,255,0.06)]" />
        </div>
      ))}
    </div>
  );
}

export function DiffPanel({ server, worktreePath, onFileClick }: DiffPanelProps) {
  const { t } = useTranslation();
  const [files, setFiles] = useState<FileChange[]>([]);
  const [summary, setSummary] = useState<StatusSummary | null>(null);
  const [branch, setBranch] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    setLoading(true);
    setError(null);
    const result = await fetchWorktreeStatus(server, worktreePath);
    if (!result.success || !result.data) {
      setError(result.error || t('diff.error.fetch_status'));
      setLoading(false);
      return;
    }
    setFiles(result.data.files);
    setSummary(result.data.summary);
    setBranch(result.data.branch);
    setLoading(false);
  }, [server, worktreePath]);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  const isClean = !loading && !error && files.length === 0;

  return (
    <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
      {/* Header */}
      <div className="px-3 pt-2.5 pb-1.5 flex items-center gap-2">
        <span className="text-[11px] font-semibold uppercase tracking-[0.05em] text-muted-foreground flex-1 truncate">
          {branch || t('diff.title')}
        </span>
        {summary && !isClean && (
          <span className="flex items-center gap-1 text-[11px] font-mono shrink-0">
            <span className="text-green-400">+{summary.totalAdditions}</span>
            <span className="text-red-400">-{summary.totalDeletions}</span>
          </span>
        )}
        <button
          onClick={fetchStatus}
          className="p-1 rounded hover:bg-[rgba(255,255,255,0.06)] text-muted-foreground hover:text-foreground transition-colors shrink-0"
          title={t('diff.refresh')}>
          <RefreshCw size={14} />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <SkeletonRows />
        ) : error ? (
          <div className="px-3 py-6 text-center">
            <p className="text-[13px] text-red-400 mb-2">{error}</p>
            <button
              onClick={fetchStatus}
              className="px-3 py-1.5 text-[12px] rounded-md border border-border bg-transparent text-foreground cursor-pointer hover:bg-accent">
              {t('common.retry')}
            </button>
          </div>
        ) : isClean ? (
          <div className="px-3 py-8 flex flex-col items-center gap-2">
            <CheckCircle size={24} className="text-green-400" />
            <span className="text-[13px] text-muted-foreground">{t('diff.empty')}</span>
          </div>
        ) : (
          <div className="py-1">
            {files.map((file) => (
              <FileRow key={file.path} file={file} onClick={onFileClick} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
