import React from 'react';
import { Box, Text } from 'ink';

interface HeaderProps {
  version?: string;
}

export default function Header({ version = '0.1' }: HeaderProps) {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box borderStyle="round" borderColor="cyan" paddingX={1}>
        <Text bold color="cyan">NomadFlow CLI</Text>
        <Text> </Text>
        <Box flexGrow={1} />
        <Text dimColor>v{version}</Text>
      </Box>
    </Box>
  );
}
