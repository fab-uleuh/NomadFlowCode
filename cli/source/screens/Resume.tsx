import React from 'react';
import { Box, Text } from 'ink';
import { Select } from '@inkjs/ui';
import type { CliState } from '../lib/types.js';

interface ResumeProps {
  state: CliState;
  onResume: () => void;
  onSkip: () => void;
}

export default function Resume({ state, onResume, onSkip }: ResumeProps) {
  const label = [state.lastRepo, state.lastFeature].filter(Boolean).join(':');

  return (
    <Box flexDirection="column">
      <Text bold>Resume previous session?</Text>
      <Text dimColor>
        Last session: <Text color="yellow">{label}</Text> on{' '}
        <Text>{state.lastServer || 'localhost'}</Text>
      </Text>
      <Box marginTop={1}>
        <Select
          options={[
            { label: 'Yes, attach tmux session', value: 'resume' },
            { label: 'No, choose another session', value: 'skip' },
          ]}
          onChange={(value) => {
            if (value === 'resume') onResume();
            else onSkip();
          }}
        />
      </Box>
    </Box>
  );
}
