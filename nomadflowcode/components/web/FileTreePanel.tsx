import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { fetchListDir } from '@/lib/server-commands';
import { RefreshCw, ChevronRight, ChevronDown, Folder, File, Loader2 } from 'lucide-react-native';
import type { Server, DirEntry } from '@shared';

interface FileTreePanelProps {
  server: Server;
  worktreePath: string;
  onFileClick: (filePath: string) => void;
}

function TreeRow({
  entry,
  depth,
  expanded,
  loadingDir,
  onToggle,
  onFileClick,
}: {
  entry: DirEntry;
  depth: number;
  expanded: boolean;
  loadingDir: boolean;
  onToggle: (path: string) => void;
  onFileClick: (path: string) => void;
}) {
  return (
    <button
      onClick={() => (entry.isDir ? onToggle(entry.path) : onFileClick(entry.path))}
      className="w-full flex items-center gap-1.5 py-1 text-left hover:bg-[rgba(255,255,255,0.04)] rounded-md group"
      style={{ paddingLeft: depth * 16 + 8 }}
      title={entry.path}>
      {entry.isDir ? (
        <>
          {loadingDir ? (
            <Loader2 size={12} className="text-muted-foreground animate-spin shrink-0" />
          ) : expanded ? (
            <ChevronDown size={12} className="text-muted-foreground shrink-0" />
          ) : (
            <ChevronRight size={12} className="text-muted-foreground shrink-0" />
          )}
          <Folder size={14} className="text-amber-400/70 shrink-0" />
        </>
      ) : (
        <>
          <span className="w-3 shrink-0" />
          <File size={14} className="text-muted-foreground shrink-0" />
        </>
      )}
      <span className="text-[13px] text-foreground truncate">{entry.name}</span>
    </button>
  );
}

function SkeletonRows() {
  return (
    <div className="px-3 py-2 flex flex-col gap-2">
      {[1, 2, 3, 4].map((i) => (
        <div key={i} className="flex items-center gap-2 animate-pulse motion-reduce:animate-none">
          <div className="w-4 h-4 rounded bg-[rgba(255,255,255,0.06)]" />
          <div className="flex-1 h-4 rounded bg-[rgba(255,255,255,0.06)]" />
        </div>
      ))}
    </div>
  );
}

const MAX_DEPTH = 20;

export function FileTreePanel({ server, worktreePath, onFileClick }: FileTreePanelProps) {
  const { t } = useTranslation();
  const [entriesCache, setEntriesCache] = useState<Map<string, DirEntry[]>>(new Map());
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const [rootLoading, setRootLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const expandedRef = useRef(expandedPaths);
  expandedRef.current = expandedPaths;
  const cacheRef = useRef(entriesCache);
  cacheRef.current = entriesCache;

  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  const loadDir = useCallback(async (relativePath: string) => {
    const result = await fetchListDir(server, worktreePath, relativePath);
    if (!mountedRef.current) return null;
    if (!result.success || !result.data) {
      return { error: result.error || t('file_tree.error') };
    }
    return { entries: result.data.entries };
  }, [server, worktreePath, t]);

  // Load root on mount
  useEffect(() => {
    setRootLoading(true);
    setError(null);
    setEntriesCache(new Map());
    setExpandedPaths(new Set());
    loadDir('').then((res) => {
      if (!mountedRef.current) return;
      if (!res || res.error) {
        setError(res?.error || t('file_tree.error'));
      } else if (res.entries) {
        setEntriesCache(new Map([['', res.entries]]));
      }
      setRootLoading(false);
    });
  }, [server, worktreePath, loadDir, t]);

  const handleToggle = useCallback(async (path: string) => {
    if (expandedRef.current.has(path)) {
      setExpandedPaths((prev) => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
      return;
    }

    // Expand — load if not cached
    setExpandedPaths((prev) => new Set(prev).add(path));
    if (cacheRef.current.has(path)) return;

    setLoadingPaths((prev) => new Set(prev).add(path));
    const res = await loadDir(path);
    if (!mountedRef.current) return;
    setLoadingPaths((prev) => {
      const next = new Set(prev);
      next.delete(path);
      return next;
    });
    if (res?.entries) {
      setEntriesCache((prev) => new Map(prev).set(path, res.entries!));
    }
  }, [loadDir]);

  const handleRefresh = useCallback(async () => {
    setRootLoading(true);
    setError(null);
    setEntriesCache(new Map());
    setExpandedPaths(new Set());
    setLoadingPaths(new Set());
    const res = await loadDir('');
    if (!mountedRef.current) return;
    if (!res || res.error) {
      setError(res?.error || t('file_tree.error'));
    } else if (res.entries) {
      setEntriesCache(new Map([['', res.entries]]));
    }
    setRootLoading(false);
  }, [loadDir, t]);

  // Recursive tree renderer
  const renderEntries = (parentPath: string, depth: number): React.ReactNode => {
    if (depth > MAX_DEPTH) return null;
    const entries = entriesCache.get(parentPath);
    if (!entries) return null;

    return entries.map((entry) => (
      <div key={entry.path}>
        <TreeRow
          entry={entry}
          depth={depth}
          expanded={expandedPaths.has(entry.path)}
          loadingDir={loadingPaths.has(entry.path)}
          onToggle={handleToggle}
          onFileClick={onFileClick}
        />
        {entry.isDir && expandedPaths.has(entry.path) && renderEntries(entry.path, depth + 1)}
      </div>
    ));
  };

  return (
    <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
      {/* Header */}
      <div className="px-3 pt-2.5 pb-1.5 flex items-center gap-2">
        <span className="text-[11px] font-semibold uppercase tracking-[0.05em] text-muted-foreground flex-1 truncate">
          {t('file_tree.title')}
        </span>
        <button
          onClick={handleRefresh}
          className="p-1 rounded hover:bg-[rgba(255,255,255,0.06)] text-muted-foreground hover:text-foreground transition-colors shrink-0"
          title={t('diff.refresh')}>
          <RefreshCw size={14} />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto py-1">
        {rootLoading ? (
          <SkeletonRows />
        ) : error ? (
          <div className="px-3 py-6 text-center">
            <p className="text-[13px] text-red-400 mb-2">{error}</p>
            <button
              onClick={handleRefresh}
              className="px-3 py-1.5 text-[12px] rounded-md border border-border bg-transparent text-foreground cursor-pointer hover:bg-accent">
              {t('common.retry')}
            </button>
          </div>
        ) : (entriesCache.get('')?.length ?? 0) === 0 ? (
          <div className="px-3 py-8 text-center">
            <span className="text-[13px] text-muted-foreground">{t('file_tree.empty')}</span>
          </div>
        ) : (
          renderEntries('', 0)
        )}
      </div>
    </div>
  );
}
