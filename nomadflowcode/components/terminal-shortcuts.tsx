import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Icon } from '@/components/ui/icon';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Text } from '@/components/ui/text';
import type { TerminalShortcut } from '@/lib/types';
import {
  ChevronDownIcon,
  ChevronUpIcon,
  PlusIcon,
  PlayIcon,
  TrashIcon,
} from 'lucide-react-native';
import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
  View,
  ScrollView,
  Pressable,
  Modal,
  Animated,
  KeyboardAvoidingView,
  Platform,
} from 'react-native';

// ---------- ShortcutQuickBar ----------

interface ShortcutQuickBarProps {
  shortcuts: TerminalShortcut[];
  onExecute: (shortcut: TerminalShortcut) => void;
  onAdd: () => void;
  onEdit: (shortcut: TerminalShortcut) => void;
}

export function ShortcutQuickBar({ shortcuts, onExecute, onAdd, onEdit }: ShortcutQuickBarProps) {
  const [collapsed, setCollapsed] = useState(true);
  const heightAnim = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    Animated.timing(heightAnim, {
      toValue: collapsed ? 0 : 1,
      duration: 200,
      useNativeDriver: false,
    }).start();
  }, [collapsed]);

  if (shortcuts.length === 0 && !collapsed) {
    return null;
  }

  const barHeight = heightAnim.interpolate({
    inputRange: [0, 1],
    outputRange: [0, 44],
  });

  const barOpacity = heightAnim.interpolate({
    inputRange: [0, 1],
    outputRange: [0, 1],
  });

  return (
    <View className="border-b border-border bg-card">
      <Animated.View style={{ height: barHeight, opacity: barOpacity, overflow: 'hidden' }}>
        <ScrollView
          horizontal
          showsHorizontalScrollIndicator={false}
          contentContainerStyle={{ alignItems: 'center', paddingHorizontal: 8, gap: 6 }}
          className="flex-1"
        >
          {shortcuts
            .sort((a, b) => a.order - b.order)
            .map((s) => (
              <Pressable
                key={s.id}
                onPress={() => onExecute(s)}
                onLongPress={() => onEdit(s)}
                className="flex-row items-center gap-1.5 rounded-full bg-primary/10 px-3 py-1.5"
              >
                {s.autoExecute && (
                  <Icon as={PlayIcon} className="text-primary" size={12} />
                )}
                <Text className="text-xs font-medium text-primary">{s.label}</Text>
              </Pressable>
            ))}
          <Pressable
            onPress={onAdd}
            style={{ minWidth: 44, minHeight: 44 }}
            className="items-center justify-center rounded-full bg-muted"
          >
            <Icon as={PlusIcon} className="text-muted-foreground" size={14} />
          </Pressable>
        </ScrollView>
      </Animated.View>

      {shortcuts.length > 0 && (
        <Pressable
          onPress={() => setCollapsed(!collapsed)}
          style={{ minHeight: 44 }}
          className="items-center justify-center"
        >
          <Icon
            as={collapsed ? ChevronDownIcon : ChevronUpIcon}
            className="text-muted-foreground"
            size={14}
          />
        </Pressable>
      )}
    </View>
  );
}

// ---------- ShortcutFormModal ----------

interface ShortcutFormModalProps {
  visible: boolean;
  shortcut?: TerminalShortcut | null;
  onSave: (data: { label: string; command: string; autoExecute: boolean }) => void;
  onDelete?: () => void;
  onClose: () => void;
}

export function ShortcutFormModal({ visible, shortcut, onSave, onDelete, onClose }: ShortcutFormModalProps) {
  const { t } = useTranslation();
  const [label, setLabel] = useState('');
  const [command, setCommand] = useState('');
  const [autoExecute, setAutoExecute] = useState(true);

  useEffect(() => {
    if (visible) {
      setLabel(shortcut?.label ?? '');
      setCommand(shortcut?.command ?? '');
      setAutoExecute(shortcut?.autoExecute ?? true);
    }
  }, [visible, shortcut]);

  const isValid = label.trim().length > 0 && command.trim().length > 0;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
        className="flex-1"
      >
        <Pressable onPress={onClose} className="flex-1 items-center justify-center bg-black/60 p-6">
          <Pressable onPress={() => {}} className="w-full max-w-sm">
            <Card>
              <CardHeader>
                <CardTitle>{shortcut ? t('terminal.shortcuts.custom.edit_title') : t('terminal.shortcuts.custom.new_title')}</CardTitle>
              </CardHeader>
              <CardContent className="gap-4">
                <View className="gap-2">
                  <Label nativeID="shortcutLabel">{t('common.label.name')}</Label>
                  <Input
                    placeholder={t('terminal.shortcuts.custom.placeholder_name')}
                    value={label}
                    onChangeText={setLabel}
                    autoCapitalize="none"
                    autoCorrect={false}
                    aria-labelledby="shortcutLabel"
                  />
                </View>

                <View className="gap-2">
                  <Label nativeID="shortcutCommand">{t('terminal.shortcuts.custom.label_command')}</Label>
                  <Input
                    placeholder={t('terminal.shortcuts.custom.placeholder_command')}
                    value={command}
                    onChangeText={setCommand}
                    autoCapitalize="none"
                    autoCorrect={false}
                    multiline
                    numberOfLines={2}
                    className="h-16"
                    aria-labelledby="shortcutCommand"
                  />
                </View>

                <Pressable
                  onPress={() => setAutoExecute(!autoExecute)}
                  className="flex-row items-center justify-between rounded-lg bg-muted p-3"
                >
                  <View className="flex-1 mr-3">
                    <Text className="font-medium">{t('terminal.shortcuts.custom.auto_execute')}</Text>
                    <Text className="text-xs text-muted-foreground">
                      {t('terminal.shortcuts.custom.auto_execute_hint')}
                    </Text>
                  </View>
                  <View
                    className={`h-6 w-11 rounded-full p-0.5 ${autoExecute ? 'bg-primary' : 'bg-muted-foreground/30'}`}
                  >
                    <View
                      className={`h-5 w-5 rounded-full bg-white ${autoExecute ? 'translate-x-5' : 'translate-x-0'}`}
                    />
                  </View>
                </Pressable>

                <View className="flex-row gap-2">
                  {shortcut && onDelete && (
                    <Button variant="destructive" className="flex-1" onPress={onDelete}>
                      <Icon as={TrashIcon} size={16} />
                      <Text className="ml-1">{t('common.delete')}</Text>
                    </Button>
                  )}
                  <Button variant="outline" className="flex-1" onPress={onClose}>
                    <Text>{t('common.cancel')}</Text>
                  </Button>
                  <Button
                    className="flex-1"
                    disabled={!isValid}
                    onPress={() => onSave({ label: label.trim(), command: command.trim(), autoExecute })}
                  >
                    <Text>{t('common.save')}</Text>
                  </Button>
                </View>
              </CardContent>
            </Card>
          </Pressable>
        </Pressable>
      </KeyboardAvoidingView>
    </Modal>
  );
}

// ---------- ShortcutsSection ----------

interface ShortcutsSectionProps {
  shortcuts: TerminalShortcut[];
  onExecute: (shortcut: TerminalShortcut) => void;
  onAdd: () => void;
  onEdit: (shortcut: TerminalShortcut) => void;
}

export function ShortcutsSection({ shortcuts, onExecute, onAdd, onEdit }: ShortcutsSectionProps) {
  const { t } = useTranslation();
  return (
    <View>
      <View className="mb-2 flex-row items-center justify-between">
        <Text className="text-xs text-muted-foreground">{t('terminal.shortcuts.custom.my_shortcuts')}</Text>
        <Pressable onPress={onAdd} style={{ minWidth: 44, minHeight: 44 }} className="items-center justify-center">
          <Icon as={PlusIcon} className="text-muted-foreground" size={16} />
        </Pressable>
      </View>
      <View className="flex-row flex-wrap">
        {shortcuts
          .sort((a, b) => a.order - b.order)
          .map((s) => (
            <Pressable
              key={s.id}
              onPress={() => onExecute(s)}
              onLongPress={() => onEdit(s)}
              style={{ minHeight: 44 }}
              className="w-1/4 items-center p-2"
            >
              <View className="mb-1 h-10 w-10 items-center justify-center rounded-lg bg-background">
                <Icon as={PlayIcon} className="text-primary" size={18} />
              </View>
              <Text className="text-xs font-medium" numberOfLines={1}>{s.label}</Text>
              {s.autoExecute && (
                <Text className="text-[10px] text-muted-foreground">{t('terminal.shortcuts.custom.auto_badge')}</Text>
              )}
            </Pressable>
          ))}
        {shortcuts.length === 0 && (
          <Pressable onPress={onAdd} className="w-1/4 items-center p-2">
            <View className="mb-1 h-10 w-10 items-center justify-center rounded-lg border border-dashed border-muted-foreground/30">
              <Icon as={PlusIcon} className="text-muted-foreground" size={18} />
            </View>
            <Text className="text-xs text-muted-foreground">{t('common.add')}</Text>
          </Pressable>
        )}
      </View>
    </View>
  );
}
