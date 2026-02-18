import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { fetchFileContent } from '@/lib/server-commands';
import { ArrowLeft, FileCode } from 'lucide-react-native';
import type { Server } from '@shared';

interface FileContentViewProps {
  server: Server;
  worktreePath: string;
  filePath: string;
  onBack: () => void;
  onViewDiff: () => void;
}

function SkeletonLines() {
  return (
    <div className="px-4 py-3 flex flex-col gap-1">
      {[80, 65, 90, 45, 70, 85, 55, 60].map((w, i) => (
        <div key={i} className="flex items-center gap-2 animate-pulse motion-reduce:animate-none">
          <div className="w-[60px] h-5 rounded bg-[rgba(255,255,255,0.06)]" />
          <div className="h-5 rounded bg-[rgba(255,255,255,0.06)]" style={{ width: `${w}%` }} />
        </div>
      ))}
    </div>
  );
}

export function FileContentView({
  server,
  worktreePath,
  filePath,
  onBack,
  onViewDiff,
}: FileContentViewProps) {
  const { t } = useTranslation();
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  const requestIdRef = useRef(0);

  const fetchContent = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);
    const result = await fetchFileContent(server, worktreePath, filePath);
    if (requestIdRef.current !== requestId) return; // stale response
    if (!result.success || !result.data) {
      setError(result.error || t('diff.error.fetch_content'));
      setLoading(false);
      return;
    }
    setContent(result.data.content);
    setLoading(false);
  }, [server, worktreePath, filePath]);

  useEffect(() => {
    setShowAll(false);
    fetchContent();
  }, [fetchContent]);

  const MAX_LINES = 3000;
  const allLines = content?.split('\n') ?? [];
  const isTruncated = allLines.length > MAX_LINES && !showAll;
  const lines = isTruncated ? allLines.slice(0, MAX_LINES) : allLines;

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-[rgba(255,255,255,0.06)] bg-[rgba(15,15,23,0.9)] shrink-0">
        <button
          onClick={onBack}
          className="p-1 rounded hover:bg-[rgba(255,255,255,0.06)] text-muted-foreground hover:text-foreground"
          title={t('diff.back_to_terminal')}>
          <ArrowLeft size={16} />
        </button>
        <span className="flex-1 text-[13px] font-mono text-foreground truncate">{filePath}</span>
        <button
          onClick={onViewDiff}
          className="flex items-center gap-1.5 px-2.5 py-1 text-[12px] rounded-md border border-border bg-transparent text-muted-foreground hover:text-foreground hover:bg-[rgba(255,255,255,0.06)] cursor-pointer">
          <FileCode size={14} />
          {t('diff.view_diff')}
        </button>
        <span className="text-[11px] text-muted-foreground">Esc</span>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {loading ? (
          <SkeletonLines />
        ) : error ? (
          <div className="px-4 py-8 text-center">
            <p className="text-[13px] text-red-400 mb-2">{error}</p>
            <button
              onClick={fetchContent}
              className="px-3 py-1.5 text-[12px] rounded-md border border-border bg-transparent text-foreground cursor-pointer hover:bg-accent">
              {t('common.retry')}
            </button>
          </div>
        ) : (
          <div className="font-mono text-[14px] leading-[20px]">
            {lines.map((line, idx) => (
              <div key={idx} className="flex">
                <span className="w-[60px] shrink-0 text-right pr-3 text-muted-foreground text-[12px] select-none">
                  {idx + 1}
                </span>
                <span className="text-foreground whitespace-pre">{line}</span>
              </div>
            ))}
            {isTruncated && (
              <div className="px-4 py-3 text-center border-t border-[rgba(255,255,255,0.06)]">
                <p className="text-[12px] text-muted-foreground mb-2">
                  {t('diff.showing_lines', { shown: MAX_LINES.toLocaleString(), total: allLines.length.toLocaleString() })}
                </p>
                <button
                  onClick={() => setShowAll(true)}
                  className="px-3 py-1.5 text-[12px] rounded-md border border-border bg-transparent text-foreground cursor-pointer hover:bg-accent">
                  {t('diff.show_all_lines')}
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
