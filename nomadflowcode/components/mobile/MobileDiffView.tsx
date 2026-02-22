import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Animated,
  Easing,
  Pressable,
  ScrollView,
  View,
  useWindowDimensions,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { Text } from '@/components/ui/text';
import { Icon } from '@/components/ui/icon';
import { fetchFileDiff, fetchFileContent } from '@/lib/server-commands';
import { getLanguageFromPath } from '@/lib/syntax-utils';
import type { Server, DiffHunk, DiffLine } from '@shared';
import { FileCodeIcon, FileTextIcon, XIcon } from 'lucide-react-native';
import { Highlight, themes } from 'prism-react-renderer';

type ViewMode = 'diff' | 'file';

interface MobileDiffViewProps {
  server: Server;
  worktreePath: string;
  filePath: string;
  onClose: () => void;
  initialMode?: ViewMode;
}

function HunkBlock({ hunk }: { hunk: DiffHunk }) {
  let oldLine = hunk.oldStart;
  let newLine = hunk.newStart;

  return (
    <View>
      {/* Hunk header */}
      <View className="border-y border-border/30 bg-muted/30 px-3 py-1">
        <Text className="font-mono text-xs text-muted-foreground">
          @@ -{hunk.oldStart},{hunk.oldLines} +{hunk.newStart},{hunk.newLines} @@
        </Text>
      </View>

      {/* Lines */}
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

        const prefix = line.type === 'addition' ? '+' : line.type === 'deletion' ? '-' : ' ';

        return (
          <View
            key={lineIdx}
            className={
              line.type === 'addition'
                ? 'flex-row bg-green-900/30'
                : line.type === 'deletion'
                  ? 'flex-row bg-red-900/30'
                  : 'flex-row'
            }>
            <Text
              className="w-10 text-right font-mono text-xs text-muted-foreground"
              style={{ paddingRight: 4 }}>
              {oldNum}
            </Text>
            <Text
              className="w-10 border-r border-border/30 text-right font-mono text-xs text-muted-foreground"
              style={{ paddingRight: 4, marginRight: 4 }}>
              {newNum}
            </Text>
            <Text
              className={
                line.type === 'addition'
                  ? 'flex-1 font-mono text-xs text-green-300'
                  : line.type === 'deletion'
                    ? 'flex-1 font-mono text-xs text-red-300'
                    : 'flex-1 font-mono text-xs text-foreground'
              }
              numberOfLines={1}>
              {prefix}
              {line.content}
            </Text>
          </View>
        );
      })}
    </View>
  );
}

function FileContentBlock({ content, language }: { content: string; language: string }) {
  return (
    <Highlight code={content} language={language} theme={themes.vsDark}>
      {({ tokens, getTokenProps }) => (
        <View>
          {tokens.map((line, i) => (
            <View key={i} className="flex-row">
              <Text
                className="w-12 text-right font-mono text-xs text-muted-foreground"
                style={{ paddingRight: 8 }}>
                {i + 1}
              </Text>
              <Text className="flex-1 font-mono text-xs" numberOfLines={1}>
                {line.map((token, key) => {
                  const props = getTokenProps({ token });
                  return (
                    <Text key={key} style={{ color: props.style?.color as string }}>
                      {props.children}
                    </Text>
                  );
                })}
              </Text>
            </View>
          ))}
        </View>
      )}
    </Highlight>
  );
}

export function MobileDiffView({ server, worktreePath, filePath, onClose, initialMode = 'diff' }: MobileDiffViewProps) {
  const { t } = useTranslation();
  const insets = useSafeAreaInsets();
  const { height: screenHeight } = useWindowDimensions();
  const [mode, setMode] = useState<ViewMode>(initialMode);
  const language = useMemo(() => getLanguageFromPath(filePath), [filePath]);
  const [hunks, setHunks] = useState<DiffHunk[]>([]);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);
  const slideAnim = useRef(new Animated.Value(1)).current;

  const fetchDiff = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);
    const result = await fetchFileDiff(server, worktreePath, filePath);
    if (requestIdRef.current !== requestId) return;
    if (!result.success || !result.data) {
      setError(result.error || t('diff.error.fetch_diff'));
      setLoading(false);
      return;
    }
    setHunks(result.data.hunks);
    setLoading(false);
  }, [server, worktreePath, filePath]);

  const fetchContent = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);
    const result = await fetchFileContent(server, worktreePath, filePath);
    if (requestIdRef.current !== requestId) return;
    if (!result.success || !result.data) {
      setError(result.error || t('diff.error.fetch_content'));
      setLoading(false);
      return;
    }
    setFileContent(result.data.content);
    setLoading(false);
  }, [server, worktreePath, filePath]);

  // Fetch when mode changes
  useEffect(() => {
    if (mode === 'diff') {
      fetchDiff();
    } else {
      fetchContent();
    }
  }, [mode, fetchDiff, fetchContent]);

  // Slide-up animation on mount
  useEffect(() => {
    Animated.timing(slideAnim, {
      toValue: 0,
      duration: 250,
      easing: Easing.out(Easing.ease),
      useNativeDriver: true,
    }).start();
  }, [slideAnim]);

  const handleClose = useCallback(() => {
    Animated.timing(slideAnim, {
      toValue: 1,
      duration: 200,
      easing: Easing.in(Easing.ease),
      useNativeDriver: true,
    }).start(() => onClose());
  }, [slideAnim, onClose]);

  const retry = mode === 'diff' ? fetchDiff : fetchContent;

  return (
    <Animated.View
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        zIndex: 300,
        transform: [
          {
            translateY: slideAnim.interpolate({
              inputRange: [0, 1],
              outputRange: [0, screenHeight],
            }),
          },
        ],
      }}
      className="bg-background">
      {/* Header */}
      <View
        className="flex-row items-center border-b border-border bg-card px-3 py-3"
        style={{ paddingTop: insets.top + 12 }}>
        <Text className="flex-1 font-mono text-sm" numberOfLines={1}>
          {filePath}
        </Text>

        {/* Toggle Diff / File */}
        <Pressable
          onPress={() => setMode(mode === 'diff' ? 'file' : 'diff')}
          className="mr-2 flex-row items-center gap-1 rounded-md border border-border px-2 py-1">
          <Icon
            as={mode === 'diff' ? FileTextIcon : FileCodeIcon}
            className="text-muted-foreground"
            size={14}
          />
          <Text className="text-xs text-muted-foreground">
            {mode === 'diff' ? t('diff.view_file') : t('diff.view_diff')}
          </Text>
        </Pressable>

        <Pressable
          onPress={handleClose}
          style={{ minWidth: 44, minHeight: 44 }}
          className="items-center justify-center">
          <Icon as={XIcon} className="text-muted-foreground" size={20} />
        </Pressable>
      </View>

      {/* Content */}
      {loading ? (
        <View className="flex-1 px-4 py-6">
          {[1, 2, 3, 4, 5, 6, 7, 8].map((i) => (
            <View key={i} className="mb-2 flex-row items-center gap-2">
              <View className="h-4 w-10 rounded bg-muted" />
              <View className="h-4 w-10 rounded bg-muted" />
              <View className="h-4 flex-1 rounded bg-muted" />
            </View>
          ))}
        </View>
      ) : error ? (
        <View className="flex-1 items-center justify-center px-4">
          <Text className="mb-3 text-sm text-destructive">{error}</Text>
          <Pressable onPress={retry} className="rounded-lg bg-muted px-4 py-2">
            <Text className="text-sm font-medium">{t('common.retry')}</Text>
          </Pressable>
        </View>
      ) : mode === 'diff' ? (
        hunks.length === 0 ? (
          <View className="flex-1 items-center justify-center">
            <Text className="text-sm text-muted-foreground">{t('diff.no_changes')}</Text>
          </View>
        ) : (
          <ScrollView className="flex-1" contentContainerStyle={{ paddingBottom: 40 }}>
            <ScrollView horizontal showsHorizontalScrollIndicator nestedScrollEnabled={true}>
              <View style={{ minWidth: '100%' }}>
                {hunks.map((hunk, hunkIdx) => (
                  <HunkBlock key={hunkIdx} hunk={hunk} />
                ))}
              </View>
            </ScrollView>
          </ScrollView>
        )
      ) : fileContent !== null ? (
        <ScrollView className="flex-1" contentContainerStyle={{ paddingBottom: 40 }}>
          <ScrollView horizontal showsHorizontalScrollIndicator nestedScrollEnabled={true}>
            <View style={{ minWidth: '100%' }}>
              <FileContentBlock content={fileContent} language={language} />
            </View>
          </ScrollView>
        </ScrollView>
      ) : (
        <View className="flex-1 items-center justify-center">
          <Text className="text-sm text-muted-foreground">{t('diff.no_content')}</Text>
        </View>
      )}
    </Animated.View>
  );
}
