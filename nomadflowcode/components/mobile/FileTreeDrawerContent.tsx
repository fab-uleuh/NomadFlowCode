import { useState, useEffect, useCallback, useRef } from 'react';
import { Pressable, ScrollView, View, ActivityIndicator } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Text } from '@/components/ui/text';
import { Icon } from '@/components/ui/icon';
import { fetchListDir } from '@/lib/server-commands';
import type { Server, DirEntry } from '@shared';
import { ChevronRightIcon, ChevronDownIcon, FolderIcon, FileIcon, RefreshCwIcon, XIcon } from 'lucide-react-native';

interface FileTreeDrawerContentProps {
  server: Server;
  worktreePath: string;
  isOpen: boolean;
  onClose: () => void;
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
    <Pressable
      onPress={() => (entry.isDir ? onToggle(entry.path) : onFileClick(entry.path))}
      className="flex-row items-center gap-1.5 py-2 active:bg-muted/50"
      style={{ paddingLeft: depth * 16 + 16, paddingRight: 16 }}>
      {entry.isDir ? (
        <>
          {loadingDir ? (
            <ActivityIndicator size={12} color="rgba(255,255,255,0.4)" />
          ) : expanded ? (
            <Icon as={ChevronDownIcon} className="text-muted-foreground" size={12} />
          ) : (
            <Icon as={ChevronRightIcon} className="text-muted-foreground" size={12} />
          )}
          <Icon as={FolderIcon} className="text-yellow-500/70" size={14} />
        </>
      ) : (
        <>
          <View style={{ width: 12 }} />
          <Icon as={FileIcon} className="text-muted-foreground" size={14} />
        </>
      )}
      <Text className="flex-1 text-sm" numberOfLines={1}>
        {entry.name}
      </Text>
    </Pressable>
  );
}

const MAX_DEPTH = 20;

export function FileTreeDrawerContent({
  server,
  worktreePath,
  isOpen,
  onClose,
  onFileClick,
}: FileTreeDrawerContentProps) {
  const { t } = useTranslation();
  const [entriesCache, setEntriesCache] = useState<Map<string, DirEntry[]>>(new Map());
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const [rootLoading, setRootLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const prevOpenRef = useRef(false);
  const prevWorktreeRef = useRef(worktreePath);
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

  const loadRoot = useCallback(async () => {
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

  // Lazy-load: fetch when drawer opens or worktreePath changes while open
  useEffect(() => {
    const justOpened = isOpen && !prevOpenRef.current;
    const worktreeChanged = isOpen && worktreePath !== prevWorktreeRef.current;
    if (justOpened || worktreeChanged) {
      loadRoot();
    }
    prevOpenRef.current = isOpen;
    prevWorktreeRef.current = worktreePath;
  }, [isOpen, worktreePath, loadRoot]);

  const handleToggle = useCallback(async (path: string) => {
    if (expandedRef.current.has(path)) {
      setExpandedPaths((prev) => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
      return;
    }

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

  const renderEntries = (parentPath: string, depth: number): React.ReactNode => {
    if (depth > MAX_DEPTH) return null;
    const entries = entriesCache.get(parentPath);
    if (!entries) return null;

    return entries.map((entry) => (
      <View key={entry.path}>
        <TreeRow
          entry={entry}
          depth={depth}
          expanded={expandedPaths.has(entry.path)}
          loadingDir={loadingPaths.has(entry.path)}
          onToggle={handleToggle}
          onFileClick={onFileClick}
        />
        {entry.isDir && expandedPaths.has(entry.path) && renderEntries(entry.path, depth + 1)}
      </View>
    ));
  };

  return (
    <View className="flex-1">
      {/* Header */}
      <View className="flex-row items-center justify-between border-b border-border px-4 py-3">
        <Text className="text-base font-semibold">{t('file_tree.title')}</Text>
        <View className="flex-row items-center gap-1">
          <Pressable
            onPress={loadRoot}
            style={{ minWidth: 44, minHeight: 44 }}
            className="items-center justify-center">
            <Icon as={RefreshCwIcon} className="text-muted-foreground" size={18} />
          </Pressable>
          <Pressable
            onPress={onClose}
            style={{ minWidth: 44, minHeight: 44 }}
            className="items-center justify-center">
            <Icon as={XIcon} className="text-muted-foreground" size={20} />
          </Pressable>
        </View>
      </View>

      {/* Content */}
      <ScrollView className="flex-1" contentContainerStyle={{ paddingBottom: 24 }}>
        {rootLoading && (
          <View className="px-4 py-3">
            {[1, 2, 3, 4].map((i) => (
              <View key={i} className="mb-3 flex-row items-center gap-2">
                <View className="h-4 w-4 animate-pulse rounded bg-muted" />
                <View className="h-4 flex-1 animate-pulse rounded bg-muted" />
              </View>
            ))}
          </View>
        )}

        {error && !rootLoading && (
          <View className="items-center px-4 py-8">
            <Text className="mb-3 text-sm text-destructive">{error}</Text>
            <Pressable onPress={loadRoot} className="rounded-lg bg-muted px-4 py-2">
              <Text className="text-sm font-medium">{t('common.retry')}</Text>
            </Pressable>
          </View>
        )}

        {!rootLoading && !error && (entriesCache.get('')?.length ?? 0) === 0 && (
          <View className="items-center px-4 py-8">
            <Text className="text-sm text-muted-foreground">{t('file_tree.empty')}</Text>
          </View>
        )}

        {!rootLoading && !error && renderEntries('', 0)}
      </ScrollView>
    </View>
  );
}
