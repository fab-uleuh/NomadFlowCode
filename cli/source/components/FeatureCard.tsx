import React from 'react';
import { Box, Text } from 'ink';
import type { Feature } from '../lib/types.js';

interface FeatureCardProps {
  feature: Feature;
  isSelected?: boolean;
}

export default function FeatureCard({ feature, isSelected }: FeatureCardProps) {
  const processLabel = feature.paneCommand
    ? feature.paneCommand.toLowerCase() === 'bash' || feature.paneCommand.toLowerCase() === 'zsh'
      ? 'idle'
      : feature.paneCommand
    : null;

  return (
    <Box gap={2}>
      <Text color={isSelected ? 'cyan' : undefined} bold={isSelected}>
        {feature.name}
      </Text>
      <Text dimColor>{feature.branch}</Text>
      {processLabel && (
        <Text color={processLabel === 'idle' ? 'gray' : 'green'}>
          {processLabel === 'idle' ? '  idle' : `● ${processLabel} running`}
        </Text>
      )}
    </Box>
  );
}
