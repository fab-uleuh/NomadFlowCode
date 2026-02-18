import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { fetchFileDiff } from '@/lib/server-commands';
import { ArrowLeft, FileText } from 'lucide-react-native';
import type { Server, DiffHunk, DiffLine } from '@shared';

interface FileDiffViewProps {
  server: Server;
  worktreePath: string;
  filePath: string;
  onBack: () => void;
  onViewFile: () => void;
}

function SkeletonLines() {
  return (
    <div className="px-4 py-3 flex flex-col gap-1">
      {[1, 2, 3, 4, 5, 6].map((i) => (
        <div key={i} className="flex items-center gap-2 animate-pulse motion-reduce:animate-none">
          <div className="w-[50px] h-5 rounded bg-[rgba(255,255,255,0.06)]" />
          <div className="w-[50px] h-5 rounded bg-[rgba(255,255,255,0.06)]" />
          <div className="flex-1 h-5 rounded bg-[rgba(255,255,255,0.06)]" />
        </div>
      ))}
    </div>
  );
}

function HunkBlock({ hunk }: { hunk: DiffHunk }) {
  let oldLine = hunk.oldStart;
  let newLine = hunk.newStart;

  return (
    <div>
      <div className="px-4 py-1 text-[12px] font-mono text-muted-foreground bg-[rgba(255,255,255,0.03)] border-y border-[rgba(255,255,255,0.04)]">
        @@ -{hunk.oldStart},{hunk.oldLines} +{hunk.newStart},{hunk.newLines} @@
      </div>
      {hunk.lines.map((line: DiffLine, lineIdx: number) => {
        let oldNum = '';
        let newNum = '';

        if (line.type === 'context') {
          oldNum = String(oldLine++);
          newNum = String(newLine++);
        } else if (line.type === 'deletion') {
          oldNum = String(oldLine++);
        } else if (line.type === 'addition') {
          newNum = String(newLine++);
        }

        const bgClass =
          line.type === 'addition'
            ? 'bg-green-900/30'
            : line.type === 'deletion'
              ? 'bg-red-900/30'
              : '';
        const textClass =
          line.type === 'addition'
            ? 'text-green-300'
            : line.type === 'deletion'
              ? 'text-red-300'
              : 'text-foreground';
        const prefix = line.type === 'addition' ? '+' : line.type === 'deletion' ? '-' : ' ';

        return (
          <div key={lineIdx} className={`flex font-mono text-[13px] leading-[20px] ${bgClass}`}>
            <span className="w-[50px] shrink-0 text-right pr-2 text-muted-foreground text-[12px] select-none">
              {oldNum}
            </span>
            <span className="w-[50px] shrink-0 text-right pr-2 text-muted-foreground text-[12px] select-none border-r border-[rgba(255,255,255,0.06)] mr-2">
              {newNum}
            </span>
            <span className={`${textClass} whitespace-pre`}>
              {prefix}
              {line.content}
            </span>
          </div>
        );
      })}
    </div>
  );
}

export function FileDiffView({
  server,
  worktreePath,
  filePath,
  onBack,
  onViewFile,
}: FileDiffViewProps) {
  const { t } = useTranslation();
  const [hunks, setHunks] = useState<DiffHunk[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);

  const fetchDiff = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);
    const result = await fetchFileDiff(server, worktreePath, filePath);
    if (requestIdRef.current !== requestId) return; // stale response
    if (!result.success || !result.data) {
      setError(result.error || t('diff.error.fetch_diff'));
      setLoading(false);
      return;
    }
    setHunks(result.data.hunks);
    setLoading(false);
  }, [server, worktreePath, filePath]);

  useEffect(() => {
    fetchDiff();
  }, [fetchDiff]);

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
          onClick={onViewFile}
          className="flex items-center gap-1.5 px-2.5 py-1 text-[12px] rounded-md border border-border bg-transparent text-muted-foreground hover:text-foreground hover:bg-[rgba(255,255,255,0.06)] cursor-pointer">
          <FileText size={14} />
          {t('diff.view_file')}
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
              onClick={fetchDiff}
              className="px-3 py-1.5 text-[12px] rounded-md border border-border bg-transparent text-foreground cursor-pointer hover:bg-accent">
              {t('common.retry')}
            </button>
          </div>
        ) : hunks.length === 0 ? (
          <div className="px-4 py-8 text-center">
            <p className="text-[13px] text-muted-foreground">{t('diff.no_changes')}</p>
          </div>
        ) : (
          hunks.map((hunk, hunkIdx) => <HunkBlock key={hunkIdx} hunk={hunk} />)
        )}
      </div>
    </div>
  );
}
