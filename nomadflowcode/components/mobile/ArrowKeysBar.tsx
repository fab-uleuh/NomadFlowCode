import { useCallback, useEffect, useRef, useState } from 'react';
import { Animated, Pressable, View } from 'react-native';

import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import type { LucideIcon } from 'lucide-react-native';
import {
  ArrowDownIcon,
  ArrowLeftIcon,
  ArrowRightIcon,
  ArrowUpIcon,
  CornerDownLeftIcon,
  XCircleIcon,
  XIcon,
} from 'lucide-react-native';

interface ArrowKeysBarProps {
  onSendKey: (sequence: string) => void;
  visible: boolean;
}

const ARROW_KEYS = [
  { label: '\u2190', sequence: '\x1b[D', icon: ArrowLeftIcon, repeat: true },
  { label: '\u2192', sequence: '\x1b[C', icon: ArrowRightIcon, repeat: true },
  { label: '\u2191', sequence: '\x1b[A', icon: ArrowUpIcon, repeat: true },
  { label: '\u2193', sequence: '\x1b[B', icon: ArrowDownIcon, repeat: true },
];

const ACTION_KEYS = [
  { label: 'Tab', sequence: '\t', icon: CornerDownLeftIcon, repeat: false },
  { label: 'Esc', sequence: '\x1b', icon: XIcon, repeat: false },
  { label: '\u2303C', sequence: '\x03', icon: XCircleIcon, repeat: false },
];

// Optional haptic feedback — degrade gracefully if expo-haptics not installed
let Haptics: any = null;
try {
  Haptics = require('expo-haptics');
} catch {}

export function ArrowKeysBar({ onSendKey, visible }: ArrowKeysBarProps) {
  const activeIntervalsRef = useRef(new Set<ReturnType<typeof setInterval>>());
  const translateY = useRef(new Animated.Value(44)).current;
  const [showContainer, setShowContainer] = useState(false);

  const clearAllRepeats = useCallback(() => {
    for (const id of activeIntervalsRef.current) {
      clearInterval(id);
    }
    activeIntervalsRef.current.clear();
  }, []);

  useEffect(() => {
    if (visible) {
      setShowContainer(true);
      Animated.timing(translateY, {
        toValue: 0,
        duration: 150,
        useNativeDriver: true,
      }).start();
    } else {
      clearAllRepeats();
      Animated.timing(translateY, {
        toValue: 44,
        duration: 150,
        useNativeDriver: true,
      }).start(({ finished }) => {
        if (finished) setShowContainer(false);
      });
    }
  }, [visible, translateY, clearAllRepeats]);

  // Also clear on unmount
  useEffect(() => clearAllRepeats, [clearAllRepeats]);

  return (
    <Animated.View
      style={{
        height: showContainer ? 44 : 0,
        overflow: 'hidden',
        transform: [{ translateY }],
      }}
      pointerEvents={visible ? 'auto' : 'none'}>
      <View
        className="flex-row items-center border-t border-border bg-card"
        style={{ height: 44 }}>
        {/* Arrow keys group */}
        <View className="flex-1 flex-row items-center justify-evenly">
          {ARROW_KEYS.map((key) => (
            <KeyButton
              key={key.label}
              icon={key.icon}
              label={key.label}
              onSendKey={onSendKey}
              sequence={key.sequence}
              repeat={key.repeat}
              activeIntervals={activeIntervalsRef.current}
            />
          ))}
        </View>

        {/* Separator */}
        <View className="h-5 w-px bg-border" />

        {/* Action keys group */}
        <View className="flex-row items-center justify-evenly" style={{ width: '42%' }}>
          {ACTION_KEYS.map((key) => (
            <KeyButton
              key={key.label}
              icon={key.icon}
              label={key.label}
              onSendKey={onSendKey}
              sequence={key.sequence}
              repeat={key.repeat}
              activeIntervals={activeIntervalsRef.current}
            />
          ))}
        </View>
      </View>
    </Animated.View>
  );
}

interface KeyButtonProps {
  icon: LucideIcon;
  label: string;
  onSendKey: (sequence: string) => void;
  sequence: string;
  repeat: boolean;
  activeIntervals: Set<ReturnType<typeof setInterval>>;
}

function KeyButton({ icon, label, onSendKey, sequence, repeat, activeIntervals }: KeyButtonProps) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | ReturnType<typeof setInterval> | null>(null);

  const clearRepeat = useCallback(() => {
    if (timerRef.current) {
      activeIntervals.delete(timerRef.current);
      clearTimeout(timerRef.current as ReturnType<typeof setTimeout>);
      clearInterval(timerRef.current as ReturnType<typeof setInterval>);
      timerRef.current = null;
    }
  }, [activeIntervals]);

  // Clean up on unmount
  useEffect(() => clearRepeat, [clearRepeat]);

  const handlePressIn = useCallback(() => {
    clearRepeat();
    onSendKey(sequence);
    if (Haptics) {
      try { Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light); } catch {}
    }
    if (repeat) {
      // Initial delay before repeat starts (like a real keyboard)
      const delayId = setTimeout(() => {
        activeIntervals.delete(delayId); // Clean up stale delay reference
        const intervalId = setInterval(() => onSendKey(sequence), 80);
        timerRef.current = intervalId;
        activeIntervals.add(intervalId);
      }, 350);
      timerRef.current = delayId;
      activeIntervals.add(delayId);
    }
  }, [onSendKey, sequence, repeat, clearRepeat, activeIntervals]);

  return (
    <Pressable
      onPressIn={handlePressIn}
      onPressOut={clearRepeat}
      style={{ minWidth: 44, minHeight: 44 }}
      className="items-center justify-center rounded active:bg-accent">
      <Icon as={icon} className="text-muted-foreground" size={16} />
      <Text className="text-[9px] text-muted-foreground">{label}</Text>
    </Pressable>
  );
}
