import { cva, type VariantProps } from 'class-variance-authority';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import { Icon } from '@/components/ui/icon';
import { Text } from '@/components/ui/text';
import { View } from 'react-native';
import { Circle, Check, X } from 'lucide-react-native';
import type { LucideIcon } from 'lucide-react-native';
import type { AgentStateKind } from '@/lib/types/session';

const STATE_CONFIG: Record<
  AgentStateKind,
  { labelKey: string; icon: LucideIcon; fill?: boolean; colorClass: string; animate: boolean }
> = {
  waiting_for_input: {
    labelKey: 'agents.state.waiting',
    icon: Circle,
    fill: true,
    colorClass: 'text-warning',
    animate: false,
  },
  waiting_for_permission: {
    labelKey: 'agents.state.permission',
    icon: Circle,
    fill: true,
    colorClass: 'text-destructive',
    animate: true,
  },
  generating: {
    labelKey: 'agents.state.generating',
    icon: Circle,
    fill: true,
    colorClass: 'text-primary',
    animate: false,
  },
  idle: {
    labelKey: 'agents.state.idle',
    icon: Circle,
    colorClass: 'text-muted-foreground',
    animate: false,
  },
  done: {
    labelKey: 'agents.state.done',
    icon: Check,
    colorClass: 'text-success',
    animate: false,
  },
  error: {
    labelKey: 'agents.state.error',
    icon: X,
    colorClass: 'text-destructive',
    animate: false,
  },
  unknown: {
    labelKey: '',
    icon: Circle,
    colorClass: 'text-muted',
    animate: false,
  },
};

const ICON_SIZE = { sm: 8, md: 10, lg: 12 } as const;

const badgeVariants = cva('flex-row items-center', {
  variants: {
    size: {
      sm: 'gap-0',
      md: 'gap-1.5',
      lg: 'gap-2',
    },
  },
  defaultVariants: {
    size: 'md',
  },
});

const labelVariants = cva('', {
  variants: {
    size: {
      md: 'text-xs',
      lg: 'text-sm',
    },
  },
  defaultVariants: {
    size: 'md',
  },
});

type AgentStatusBadgeProps = VariantProps<typeof badgeVariants> & {
  state: AgentStateKind;
  agentName?: string;
  className?: string;
};

function AgentStatusBadge({ state, size, agentName, className }: AgentStatusBadgeProps) {
  const { t } = useTranslation();
  const config = STATE_CONFIG[state] ?? STATE_CONFIG.unknown;
  const label = config.labelKey ? t(config.labelKey) : '\u2014';

  return (
    <View
      className={cn(badgeVariants({ size }), className)}
      role="status"
      aria-label={`${t('agents.label')}${agentName ? ` ${agentName}` : ''}: ${label}`}
      accessibilityLabel={`${t('agents.label')}${agentName ? ` ${agentName}` : ''}: ${label}`}>
      <Icon
        as={config.icon}
        size={ICON_SIZE[size || 'md']}
        className={cn(
          config.colorClass,
          config.animate && 'animate-pulse-dot motion-reduce:animate-none'
        )}
        {...(config.fill ? { fill: 'currentColor' } : {})}
      />
      {size !== 'sm' && (
        <Text className={cn(labelVariants({ size }), 'text-muted-foreground')}>{label}</Text>
      )}
    </View>
  );
}

export { AgentStatusBadge, badgeVariants };
export type { AgentStatusBadgeProps };
