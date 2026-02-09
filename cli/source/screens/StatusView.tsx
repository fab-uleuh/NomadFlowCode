import React from 'react';
import { Box, Text } from 'ink';
import { isTmuxInstalled, sessionExists, listWindows, getPaneCommand } from '../lib/tmux.js';

interface StatusViewProps {
  tmuxSession: string;
}

export default function StatusView({ tmuxSession }: StatusViewProps) {
  if (!isTmuxInstalled()) {
    return <Text color="red">tmux is not installed</Text>;
  }

  if (!sessionExists(tmuxSession)) {
    return (
      <Box flexDirection="column">
        <Text>Session: <Text bold>{tmuxSession}</Text></Text>
        <Text dimColor>No active session</Text>
      </Box>
    );
  }

  const windows = listWindows(tmuxSession);

  return (
    <Box flexDirection="column">
      <Text bold>Session: <Text color="cyan">{tmuxSession}</Text></Text>
      <Text dimColor>{windows.length} window(s)</Text>
      <Box flexDirection="column" marginTop={1}>
        {windows.map(w => {
          const cmd = getPaneCommand(tmuxSession, w.name);
          const isIdle = !cmd || ['bash', 'zsh', 'sh', 'fish'].includes(cmd.toLowerCase());
          return (
            <Box key={w.index} gap={2}>
              <Text>{w.active ? '>' : ' '}</Text>
              <Text color={w.active ? 'cyan' : undefined}>{w.index}: {w.name}</Text>
              {cmd && (
                <Text color={isIdle ? 'gray' : 'green'}>
                  {isIdle ? 'idle' : `● ${cmd}`}
                </Text>
              )}
            </Box>
          );
        })}
      </Box>
    </Box>
  );
}
