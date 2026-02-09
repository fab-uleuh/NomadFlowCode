import React, { useState, useEffect } from 'react';
import { Box, Text } from 'ink';
import { Select, Spinner } from '@inkjs/ui';
import type { Server } from '../types.js';
import { checkHealth } from '../lib/api-client.js';

interface ServerPickerProps {
  servers: Server[];
  onSelect: (server: Server) => void;
}

export default function ServerPicker({ servers, onSelect }: ServerPickerProps) {
  const [healthMap, setHealthMap] = useState<Record<string, boolean>>({});
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    let cancelled = false;
    async function check() {
      const results: Record<string, boolean> = {};
      await Promise.all(
        servers.map(async (s) => {
          results[s.id] = await checkHealth(s);
        }),
      );
      if (!cancelled) {
        setHealthMap(results);
        setChecking(false);
      }
    }
    check();
    return () => { cancelled = true; };
  }, [servers]);

  if (checking) {
    return (
      <Box>
        <Spinner label="Checking servers..." />
      </Box>
    );
  }

  const options = servers.map(s => ({
    label: `${s.name} (${s.apiUrl || 'no url'})  ${healthMap[s.id] ? '✓ connected' : '✗ offline'}`,
    value: s.id,
  }));

  return (
    <Box flexDirection="column">
      <Text bold>Select a server:</Text>
      <Select
        options={options}
        onChange={(value) => {
          const server = servers.find(s => s.id === value);
          if (server) onSelect(server);
        }}
      />
    </Box>
  );
}
