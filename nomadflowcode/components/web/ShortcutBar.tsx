import { useTranslation } from 'react-i18next';
import { ArrowUp, ArrowDown } from 'lucide-react-native';
import type { LucideIcon } from 'lucide-react-native';

interface ShortcutBarProps {
  onSend: (data: string) => void;
  onToggleDiff?: () => void;
}

const KEYBOARD_SHORTCUTS: { label: string; char: string; Icon?: LucideIcon }[] = [
  { label: 'Ctrl+C', char: '\x03' },
  { label: 'Ctrl+D', char: '\x04' },
  { label: 'Ctrl+Z', char: '\x1a' },
  { label: 'Ctrl+L', char: '\x0c' },
  { label: 'Tab', char: '\t' },
  { label: 'Esc', char: '\x1b' },
  { label: 'Up', char: '\x1b[A', Icon: ArrowUp },
  { label: 'Down', char: '\x1b[B', Icon: ArrowDown },
];

const TMUX_SHORTCUTS = [
  { labelKey: 'terminal.shortcuts.tmux.windows', key: 'w' },
  { labelKey: 'terminal.shortcuts.tmux.new', key: 'c' },
  { labelKey: 'terminal.shortcuts.tmux.split_h', key: '"' },
  { labelKey: 'terminal.shortcuts.tmux.split_v', key: '%' },
  { labelKey: 'terminal.shortcuts.tmux.next', key: 'n' },
  { labelKey: 'terminal.shortcuts.tmux.prev', key: 'p' },
  { labelKey: 'terminal.shortcuts.tmux.detach', key: 'd' },
  { labelKey: 'terminal.shortcuts.tmux.scroll', key: '[' },
];

export function ShortcutBar({ onSend, onToggleDiff }: ShortcutBarProps) {
  const { t } = useTranslation();

  const TERMINAL_HINTS = [
    { label: t('terminal.shortcuts.hint.split_h'), key: 'split_h' },
    { label: t('terminal.shortcuts.hint.split_v'), key: 'split_v' },
    { label: t('terminal.shortcuts.hint.close'), key: 'close' },
    { label: t('terminal.shortcuts.hint.search'), key: 'search' },
    { label: t('terminal.shortcuts.hint.toggle_diff'), key: 'toggle_diff' },
  ];

  return (
    <div className="flex items-center gap-1 px-2 py-1 border-t border-border bg-card overflow-x-auto shrink-0">
      {KEYBOARD_SHORTCUTS.map((s) => (
        <button
          key={s.label}
          onClick={() => onSend(s.char)}
          className="px-2.5 py-1 border border-border rounded-md bg-secondary text-foreground text-xs font-mono whitespace-nowrap cursor-pointer hover:bg-accent flex items-center justify-center"
          title={s.label}
          aria-label={s.label}>
          {s.Icon ? <s.Icon size={12} /> : s.label}
        </button>
      ))}

      <span className="w-px h-5 bg-border mx-1 shrink-0" />

      <span className="text-[11px] text-muted-foreground shrink-0">{t('terminal.shortcuts.tmux_label')}</span>
      {TMUX_SHORTCUTS.map((s) => (
        <button
          key={s.key}
          onClick={() => onSend('\x02' + s.key)}
          className="px-2.5 py-1 border border-border rounded-md bg-secondary text-foreground text-xs font-mono whitespace-nowrap cursor-pointer hover:bg-accent"
          title={`^b ${s.key}`}>
          {t(s.labelKey)}
        </button>
      ))}

      <span className="w-px h-5 bg-border mx-1 shrink-0" />

      <span className="text-[11px] text-muted-foreground shrink-0">{t('terminal.shortcuts.label')}</span>
      {TERMINAL_HINTS.map((s) => {
        const isClickable = s.key === 'toggle_diff' && onToggleDiff;

        return isClickable ? (
          <button
            key={s.key}
            onClick={onToggleDiff}
            className="px-2.5 py-1 border border-border rounded-md bg-secondary text-muted-foreground text-xs font-mono whitespace-nowrap cursor-pointer hover:bg-accent hover:text-foreground">
            {s.label}
          </button>
        ) : (
          <span
            key={s.key}
            className="px-2.5 py-1 border border-border rounded-md bg-secondary text-muted-foreground text-xs font-mono whitespace-nowrap">
            {s.label}
          </span>
        );
      })}
    </div>
  );
}
